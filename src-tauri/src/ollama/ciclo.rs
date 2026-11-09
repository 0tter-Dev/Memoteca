//! Ciclo de vida do Ollama (E06): detectar, reutilizar, iniciar e encerrar.
//!
//! Regras (ver o `PLANO.md`):
//!
//! * Se já existe um Ollama respondendo na porta, ele é **reutilizado e nunca encerrado**.
//! * Se outro programa ocupa a porta, **nada é iniciado**.
//! * Só o processo que o próprio Memoteca iniciou é encerrado, e ele morre junto com o app,
//!   inclusive em fechamento forçado (Job Object, em `processo.rs`).

use std::time::Duration;

use serde::Serialize;
use tokio::time::Instant;

use super::erro::ErroOllama;
use super::instalacao::{DetectorDeInstalacao, DetectorReal};
use super::processo::{IniciadorDeProcesso, IniciadorReal, ProcessoFilho};
use super::tipos::InfoVersao;
use super::{ClienteOllama, ENDERECO_PADRAO};

/// Quanto o app espera o servidor responder depois de iniciá-lo, e de quanto em quanto
/// tempo pergunta.
#[derive(Debug, Clone, Copy)]
pub struct ConfigCiclo {
    pub espera_pela_inicializacao: Duration,
    pub intervalo_entre_consultas: Duration,
}

impl Default for ConfigCiclo {
    fn default() -> Self {
        Self {
            espera_pela_inicializacao: Duration::from_secs(30),
            intervalo_entre_consultas: Duration::from_millis(250),
        }
    }
}

/// Quem iniciou o Ollama que está respondendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origem {
    /// Já estava rodando (por exemplo, o app oficial iniciado com o Windows). Nunca é
    /// encerrado pelo Memoteca.
    Externo,
    /// Foi iniciado pelo Memoteca, que o encerra ao fechar.
    IniciadoPeloApp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Situacao {
    pub versao: InfoVersao,
    pub origem: Origem,
    pub modelos_instalados: usize,
}

impl Situacao {
    /// Aviso amigável quando a versão está abaixo da mínima. Não bloqueia o uso.
    pub fn aviso_de_versao(&self) -> Option<ErroOllama> {
        self.versao.aviso()
    }

    /// Não há nenhum modelo baixado, e o onboarding precisa levar o usuário a escolher um.
    pub fn precisa_de_modelo(&self) -> bool {
        self.modelos_instalados == 0
    }
}

/// Estado do Ollama na inicialização (ver "Estados de inicialização" no `PLANO.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoOllama {
    /// Há um Ollama respondendo e pronto para uso.
    EmExecucao(Situacao),
    /// Não há Ollama instalado: o onboarding leva o usuário a instalá-lo.
    NaoInstalado,
    /// Outro programa ocupa a porta. Nada foi iniciado.
    PortaOcupada,
    /// Não foi possível chegar a um Ollama em execução.
    Indisponivel(ErroOllama),
}

/// Formato que a interface recebe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EstadoParaInterface {
    /// `em_execucao`, `nao_instalado`, `porta_ocupada` ou `indisponivel`.
    pub estado: &'static str,
    pub versao: Option<String>,
    /// `externo` ou `iniciado_pelo_app`.
    pub origem: Option<&'static str>,
    #[serde(rename = "modelosInstalados")]
    pub modelos_instalados: Option<usize>,
    /// Aviso de versão antiga, quando houver.
    pub aviso: Option<String>,
    /// Mensagem amigável quando o Ollama não está utilizável.
    pub mensagem: Option<String>,
}

impl EstadoOllama {
    pub fn para_interface(&self) -> EstadoParaInterface {
        let vazio = |estado, mensagem: Option<String>| EstadoParaInterface {
            estado,
            versao: None,
            origem: None,
            modelos_instalados: None,
            aviso: None,
            mensagem,
        };

        match self {
            Self::EmExecucao(situacao) => EstadoParaInterface {
                estado: "em_execucao",
                versao: Some(situacao.versao.versao.to_string()),
                origem: Some(match situacao.origem {
                    Origem::Externo => "externo",
                    Origem::IniciadoPeloApp => "iniciado_pelo_app",
                }),
                modelos_instalados: Some(situacao.modelos_instalados),
                aviso: situacao.aviso_de_versao().map(|e| e.mensagem_amigavel()),
                mensagem: None,
            },
            Self::NaoInstalado => vazio(
                "nao_instalado",
                Some("O Ollama não está instalado neste computador.".to_owned()),
            ),
            Self::PortaOcupada => vazio(
                "porta_ocupada",
                Some(ErroOllama::PortaOcupada.mensagem_amigavel()),
            ),
            Self::Indisponivel(erro) => vazio("indisponivel", Some(erro.mensagem_amigavel())),
        }
    }
}

pub struct GerenciadorOllama {
    cliente: ClienteOllama,
    detector: Box<dyn DetectorDeInstalacao>,
    iniciador: Box<dyn IniciadorDeProcesso>,
    config: ConfigCiclo,
    /// O processo que **nós** iniciamos. Nunca guarda um Ollama externo.
    filho: Option<Box<dyn ProcessoFilho>>,
}

impl GerenciadorOllama {
    /// Gerenciador para o Ollama local, com detecção e processo reais.
    pub fn novo() -> Result<Self, ErroOllama> {
        Ok(Self::com_componentes(
            ClienteOllama::com_endereco(ENDERECO_PADRAO, super::Limites::default())?,
            Box::new(DetectorReal),
            Box::new(IniciadorReal),
            ConfigCiclo::default(),
        ))
    }

    /// Gerenciador com componentes próprios. Os testes usam versões falsas.
    pub fn com_componentes(
        cliente: ClienteOllama,
        detector: Box<dyn DetectorDeInstalacao>,
        iniciador: Box<dyn IniciadorDeProcesso>,
        config: ConfigCiclo,
    ) -> Self {
        Self {
            cliente,
            detector,
            iniciador,
            config,
            filho: None,
        }
    }

    /// Há um processo iniciado por este gerenciador e ainda rodando.
    pub fn iniciado_pelo_app(&mut self) -> bool {
        self.esquecer_filho_que_saiu();
        self.filho.is_some()
    }

    /// Descobre o estado do Ollama e, se ele estiver instalado e parado, o inicia.
    pub async fn preparar(&mut self) -> EstadoOllama {
        self.esquecer_filho_que_saiu();

        match self.cliente.verificar().await {
            Ok(info) => self.descrever(info).await,
            Err(ErroOllama::PortaOcupada) => EstadoOllama::PortaOcupada,
            Err(ErroOllama::OllamaForaDoAr) => self.iniciar_e_esperar().await,
            Err(outro) => EstadoOllama::Indisponivel(outro),
        }
    }

    /// Encerra o Ollama **se, e somente se,** foi o Memoteca que o iniciou. Um Ollama
    /// externo nunca é tocado.
    pub fn encerrar_se_iniciado_pelo_app(&mut self) {
        if let Some(mut filho) = self.filho.take() {
            let _ = filho.encerrar();
        }
    }

    // ----- Apoio -----

    fn esquecer_filho_que_saiu(&mut self) {
        let saiu = match self.filho.as_mut() {
            Some(filho) => !matches!(filho.saiu(), Ok(None)),
            None => false,
        };
        if saiu {
            self.filho = None;
        }
    }

    async fn descrever(&mut self, info: InfoVersao) -> EstadoOllama {
        let modelos = match self.cliente.listar_modelos().await {
            Ok(modelos) => modelos,
            Err(erro) => return EstadoOllama::Indisponivel(erro),
        };
        // Se o processo que iniciamos já saiu, quem responde é outro Ollama.
        self.esquecer_filho_que_saiu();
        let origem = if self.filho.is_some() {
            Origem::IniciadoPeloApp
        } else {
            Origem::Externo
        };

        EstadoOllama::EmExecucao(Situacao {
            versao: info,
            origem,
            modelos_instalados: modelos.len(),
        })
    }

    async fn iniciar_e_esperar(&mut self) -> EstadoOllama {
        let Some(executavel) = self.detector.encontrar() else {
            return EstadoOllama::NaoInstalado;
        };

        if self.filho.is_none() {
            match self.iniciador.iniciar(&executavel, &["serve"]) {
                Ok(filho) => self.filho = Some(filho),
                Err(erro) => {
                    return EstadoOllama::Indisponivel(ErroOllama::FalhaAoIniciar(
                        erro.to_string(),
                    ));
                }
            }
        }

        let limite = Instant::now() + self.config.espera_pela_inicializacao;
        // O Ollama recém-iniciado abre a porta alguns segundos antes de responder (com a
        // pasta de modelos vazia isso passa de 3 s, medido com a 0.32.9). Nesse intervalo a
        // resposta parece "outro programa". Por isso, enquanto o processo que o próprio app
        // iniciou está vivo, `PortaOcupada` é tratada como "ainda subindo", e só vale se o
        // processo sair ou se o prazo acabar com a porta ainda ocupada.
        let mut porta_ocupada_na_ultima_vez: bool;
        loop {
            match self.cliente.verificar().await {
                Ok(info) => return self.descrever(info).await,
                Err(ErroOllama::OllamaForaDoAr) => porta_ocupada_na_ultima_vez = false,
                Err(ErroOllama::PortaOcupada) => porta_ocupada_na_ultima_vez = true,
                Err(outro) => {
                    self.encerrar_se_iniciado_pelo_app();
                    return EstadoOllama::Indisponivel(outro);
                }
            }

            if let Some(filho) = self.filho.as_mut()
                && let Ok(Some(codigo)) = filho.saiu()
            {
                self.filho = None;
                // Se outro programa responde na porta e o nosso processo saiu, foi esse
                // programa que impediu o Ollama de subir.
                return if porta_ocupada_na_ultima_vez {
                    EstadoOllama::PortaOcupada
                } else {
                    EstadoOllama::Indisponivel(ErroOllama::FalhaAoIniciar(format!(
                        "o processo terminou com o código {codigo}"
                    )))
                };
            }

            if Instant::now() >= limite {
                self.encerrar_se_iniciado_pelo_app();
                return if porta_ocupada_na_ultima_vez {
                    EstadoOllama::PortaOcupada
                } else {
                    EstadoOllama::Indisponivel(ErroOllama::TempoEsgotado)
                };
            }
            tokio::time::sleep(self.config.intervalo_entre_consultas).await;
        }
    }
}

impl Drop for GerenciadorOllama {
    fn drop(&mut self) {
        self.encerrar_se_iniciado_pelo_app();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ollama::versao::{VERSAO_MINIMA, Versao};

    fn situacao(versao: Versao, modelos: usize) -> Situacao {
        Situacao {
            versao: InfoVersao {
                versao,
                atende_o_minimo: versao >= VERSAO_MINIMA,
            },
            origem: Origem::Externo,
            modelos_instalados: modelos,
        }
    }

    #[test]
    fn a_interface_recebe_o_estado_em_execucao_sem_aviso_quando_a_versao_basta() {
        let estado = EstadoOllama::EmExecucao(situacao(Versao::nova(0, 32, 9), 2));

        let dto = estado.para_interface();

        assert_eq!(dto.estado, "em_execucao");
        assert_eq!(dto.versao.as_deref(), Some("0.32.9"));
        assert_eq!(dto.origem, Some("externo"));
        assert_eq!(dto.modelos_instalados, Some(2));
        assert_eq!(dto.aviso, None);
        assert_eq!(dto.mensagem, None);
    }

    #[test]
    fn versao_antiga_vira_aviso_e_continua_em_execucao() {
        let estado = EstadoOllama::EmExecucao(situacao(Versao::nova(0, 29, 0), 0));

        let dto = estado.para_interface();

        assert_eq!(dto.estado, "em_execucao");
        assert!(dto.aviso.unwrap().contains("0.29.0"));
    }

    #[test]
    fn estados_sem_ollama_trazem_mensagem_amigavel() {
        let nao_instalado = EstadoOllama::NaoInstalado.para_interface();
        assert_eq!(nao_instalado.estado, "nao_instalado");
        assert!(
            nao_instalado
                .mensagem
                .unwrap()
                .contains("não está instalado")
        );

        let ocupada = EstadoOllama::PortaOcupada.para_interface();
        assert_eq!(ocupada.estado, "porta_ocupada");
        assert!(ocupada.mensagem.unwrap().contains("Outro programa"));

        let indisponivel =
            EstadoOllama::Indisponivel(ErroOllama::FalhaAoIniciar("código 0xC0000005".into()))
                .para_interface();
        assert_eq!(indisponivel.estado, "indisponivel");
        assert!(!indisponivel.mensagem.unwrap().contains("0xC0000005"));
    }

    #[test]
    fn o_estado_serializa_com_os_nomes_que_a_interface_espera() {
        let dto = EstadoOllama::EmExecucao(situacao(Versao::nova(0, 32, 9), 1)).para_interface();

        let json = serde_json::to_value(dto).unwrap();

        assert_eq!(json["estado"], "em_execucao");
        assert_eq!(json["modelosInstalados"], 1);
        assert_eq!(json["origem"], "externo");
    }

    #[test]
    fn sem_modelos_a_situacao_pede_um_modelo() {
        assert!(situacao(Versao::nova(0, 32, 9), 0).precisa_de_modelo());
        assert!(!situacao(Versao::nova(0, 32, 9), 1).precisa_de_modelo());
    }
}
