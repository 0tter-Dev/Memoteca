//! Instalação silenciosa do Ollama (E07).
//!
//! Fluxo de `InstaladorDoOllama::instalar`:
//!
//! 1. recusa se já existe um Ollama instalado (nunca instala por cima);
//! 2. consulta a release oficial (URL, tamanho e SHA-256);
//! 3. confere o espaço em disco e **bloqueia antes de baixar** se faltar;
//! 4. baixa o instalador, com progresso, calculando o SHA-256;
//! 5. confere tamanho, SHA-256 e a assinatura digital (publicador "Ollama Inc.");
//! 6. executa o instalador no modo totalmente silencioso, sem elevar;
//! 7. confere que o Ollama passou a existir.
//!
//! Qualquer falha, cancelamento ou sucesso **remove o instalador baixado**. Uma instalação
//! parcial é desfeita (rollback): primeiro pelo desinstalador, depois removendo a pasta de
//! instalação, mas só se ela não existia antes da tentativa.

pub mod assinatura;
pub mod disco;
pub mod erro;
pub mod executor;
pub mod fonte;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::banco::id::novo_id;
use crate::ollama::instalacao::DetectorDeInstalacao;

use assinatura::{FalhaDeAssinatura, PUBLICADOR_DO_OLLAMA, ValidadorDeAssinatura};
use disco::{EspacoEmDisco, mesma_unidade};
pub use erro::{ErroInstalacao, FalhaInstalacao};
use executor::{ErroDeExecucao, ExecutorDoInstalador};
use fonte::{Baixado, RegrasDeOrigem};

/// Endereço da release mais recente do Ollama (a mesma de ollama.com/download).
pub const URL_DA_RELEASE: &str = "https://api.github.com/repos/ollama/ollama/releases/latest";

/// Espaço necessário para a instalação em si, segundo a documentação do Ollama no Windows
/// ("pelo menos 4 GB para os binários"). Os modelos exigem espaço à parte.
pub const ESPACO_PARA_INSTALAR_EM_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fase {
    Consultando,
    VerificandoEspaco,
    Baixando,
    Verificando,
    Instalando,
    Concluida,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressoInstalacao {
    pub fase: Fase,
    pub baixado_em_bytes: u64,
    pub total_em_bytes: u64,
}

impl ProgressoInstalacao {
    fn da_fase(fase: Fase) -> Self {
        Self {
            fase,
            baixado_em_bytes: 0,
            total_em_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoInstalacao {
    /// Instalado. Traz o caminho do `ollama.exe`.
    Concluida { executavel: PathBuf },
    /// O usuário cancelou. Nada ficou para trás.
    Cancelada,
}

#[derive(Debug, Clone)]
pub struct ConfigInstalador {
    /// API de releases (devolve a lista de arquivos, tamanhos e SHA-256).
    pub url_da_release: String,
    pub regras_de_origem: RegrasDeOrigem,
    /// Onde o instalador é baixado. Fica sempre dentro da pasta do usuário.
    pub pasta_temporaria: PathBuf,
    /// Pasta onde o Ollama é instalado (a padrão, por usuário).
    pub pasta_de_instalacao: PathBuf,
    pub publicador_esperado: String,
    /// Tempo máximo para estabelecer conexão e para consultas curtas.
    pub prazo_de_consulta: Duration,
    /// Tempo máximo sem receber nenhum byte durante o download.
    pub ociosidade_do_download: Duration,
}

impl ConfigInstalador {
    /// Configuração de produção, ou `None` se o Windows não informar as pastas do usuário.
    pub fn padrao() -> Option<Self> {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
        Some(Self {
            url_da_release: URL_DA_RELEASE.to_owned(),
            regras_de_origem: RegrasDeOrigem {
                hosts_permitidos: vec!["github.com".to_owned()],
                exigir_https: true,
            },
            pasta_temporaria: std::env::temp_dir().join("Memoteca"),
            pasta_de_instalacao: local.join("Programs").join("Ollama"),
            publicador_esperado: PUBLICADOR_DO_OLLAMA.to_owned(),
            prazo_de_consulta: Duration::from_secs(20),
            ociosidade_do_download: Duration::from_secs(60),
        })
    }
}

pub struct InstaladorDoOllama {
    config: ConfigInstalador,
    http: reqwest::Client,
    detector: Arc<dyn DetectorDeInstalacao>,
    disco: Arc<dyn EspacoEmDisco>,
    assinatura: Arc<dyn ValidadorDeAssinatura>,
    executor: Arc<dyn ExecutorDoInstalador>,
}

/// Por que o fluxo parou antes de terminar.
enum Interrupcao {
    Falha(ErroInstalacao),
    Cancelado,
}

impl From<ErroInstalacao> for Interrupcao {
    fn from(erro: ErroInstalacao) -> Self {
        Self::Falha(erro)
    }
}

/// Arquivo do instalador baixado. É apagado ao sair de escopo, em qualquer desfecho.
struct ArquivoTemporario {
    parcial: PathBuf,
    final_: PathBuf,
}

impl ArquivoTemporario {
    fn novo(pasta: &Path) -> Self {
        let base = format!("OllamaSetup-{}", novo_id());
        Self {
            parcial: pasta.join(format!("{base}.parcial")),
            final_: pasta.join(format!("{base}.exe")),
        }
    }
}

impl Drop for ArquivoTemporario {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.parcial);
        let _ = std::fs::remove_file(&self.final_);
    }
}

impl InstaladorDoOllama {
    pub fn com_componentes(
        config: ConfigInstalador,
        http: reqwest::Client,
        detector: Arc<dyn DetectorDeInstalacao>,
        disco: Arc<dyn EspacoEmDisco>,
        assinatura: Arc<dyn ValidadorDeAssinatura>,
        executor: Arc<dyn ExecutorDoInstalador>,
    ) -> Self {
        Self {
            config,
            http,
            detector,
            disco,
            assinatura,
            executor,
        }
    }

    /// Instalador de produção. O cliente HTTP usa TLS do Windows e respeita o proxy do
    /// sistema.
    pub fn novo(detector: Arc<dyn DetectorDeInstalacao>) -> Result<Self, ErroInstalacao> {
        let config = ConfigInstalador::padrao().ok_or_else(|| {
            ErroInstalacao::Interno("não foi possível localizar a pasta do usuário".into())
        })?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("Memoteca/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(config.prazo_de_consulta)
            .build()
            .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;
        Ok(Self::com_componentes(
            config,
            http,
            detector,
            Arc::new(disco::EspacoReal),
            Arc::new(assinatura::ValidadorPowerShell),
            Arc::new(executor::ExecutorReal::default()),
        ))
    }

    /// Baixa, valida e instala o Ollama. Ver a documentação do módulo.
    pub async fn instalar(
        &self,
        cancelamento: &CancellationToken,
        ao_progredir: &mut (dyn FnMut(ProgressoInstalacao) + Send),
    ) -> Result<ResultadoInstalacao, FalhaInstalacao> {
        let sem_restos = |erro| FalhaInstalacao {
            erro,
            restos_na_instalacao: false,
        };

        if self.detector.encontrar().is_some() {
            return Err(sem_restos(ErroInstalacao::JaInstalado));
        }

        let existia_antes = self.config.pasta_de_instalacao.exists();
        let arquivo = ArquivoTemporario::novo(&self.config.pasta_temporaria);

        // Fases até o instalador estar baixado e validado. Nada foi instalado ainda, então
        // não há o que desfazer; o arquivo baixado é apagado por `arquivo` ao sair.
        match self
            .obter_instalador_validado(&arquivo, cancelamento, ao_progredir)
            .await
        {
            Ok(()) => {}
            Err(Interrupcao::Cancelado) => return Ok(ResultadoInstalacao::Cancelada),
            Err(Interrupcao::Falha(erro)) => return Err(sem_restos(erro)),
        }

        // Instalação.
        ao_progredir(ProgressoInstalacao::da_fase(Fase::Instalando));
        let executor = Arc::clone(&self.executor);
        let instalador = arquivo.final_.clone();
        let token = cancelamento.clone();
        let execucao = tokio::task::spawn_blocking(move || executor.instalar(&instalador, &token))
            .await
            .unwrap_or_else(|erro| Err(ErroDeExecucao::NaoIniciou(erro.to_string())));

        let falha = match execucao {
            Ok(()) => match self.detector.encontrar() {
                Some(executavel) => {
                    ao_progredir(ProgressoInstalacao::da_fase(Fase::Concluida));
                    return Ok(ResultadoInstalacao::Concluida { executavel });
                }
                None => Some(ErroInstalacao::InstalacaoNaoDetectada),
            },
            Err(ErroDeExecucao::Cancelado) => None,
            Err(ErroDeExecucao::ElevacaoNecessaria) => Some(ErroInstalacao::ElevacaoNecessaria),
            Err(ErroDeExecucao::SaiuComCodigo(codigo)) => {
                Some(ErroInstalacao::InstaladorFalhou(codigo))
            }
            Err(ErroDeExecucao::PrazoEsgotado) => Some(ErroInstalacao::TempoEsgotado),
            Err(ErroDeExecucao::NaoIniciou(detalhe)) => {
                Some(ErroInstalacao::NaoFoiPossivelExecutar(detalhe))
            }
        };

        // Falha ou cancelamento no meio da instalação: desfaz o que ficou.
        let restos = self.desfazer_instalacao_parcial(existia_antes).await;
        match falha {
            None => Ok(ResultadoInstalacao::Cancelada),
            Some(erro) => Err(FalhaInstalacao {
                erro,
                restos_na_instalacao: restos,
            }),
        }
    }

    // ----- Fases -----

    async fn obter_instalador_validado(
        &self,
        arquivo: &ArquivoTemporario,
        cancelamento: &CancellationToken,
        ao_progredir: &mut (dyn FnMut(ProgressoInstalacao) + Send),
    ) -> Result<(), Interrupcao> {
        ao_progredir(ProgressoInstalacao::da_fase(Fase::Consultando));
        let consulta = fonte::consultar_release(
            &self.http,
            &self.config.url_da_release,
            self.config.prazo_de_consulta,
            &self.config.regras_de_origem,
        );
        let release = tokio::select! {
            biased;
            () = cancelamento.cancelled() => return Err(Interrupcao::Cancelado),
            resultado = consulta => resultado?,
        };

        ao_progredir(ProgressoInstalacao::da_fase(Fase::VerificandoEspaco));
        self.verificar_espaco(release.tamanho_em_bytes)?;

        std::fs::create_dir_all(&self.config.pasta_temporaria)
            .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;

        ao_progredir(ProgressoInstalacao {
            fase: Fase::Baixando,
            baixado_em_bytes: 0,
            total_em_bytes: release.tamanho_em_bytes,
        });
        let baixado = fonte::baixar(
            &self.http,
            &release,
            &arquivo.parcial,
            self.config.ociosidade_do_download,
            cancelamento,
            &mut |baixado, total| {
                ao_progredir(ProgressoInstalacao {
                    fase: Fase::Baixando,
                    baixado_em_bytes: baixado,
                    total_em_bytes: total,
                });
            },
        )
        .await?;
        let sha256 = match baixado {
            Baixado::Cancelado => return Err(Interrupcao::Cancelado),
            Baixado::Completo { sha256 } => sha256,
        };

        ao_progredir(ProgressoInstalacao::da_fase(Fase::Verificando));
        if sha256 != release.sha256 {
            return Err(ErroInstalacao::InstaladorCorrompido(
                "o SHA-256 do arquivo não confere com o da release".into(),
            )
            .into());
        }
        std::fs::rename(&arquivo.parcial, &arquivo.final_)
            .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;

        let validador = Arc::clone(&self.assinatura);
        let caminho = arquivo.final_.clone();
        let publicador = self.config.publicador_esperado.clone();
        let assinatura =
            tokio::task::spawn_blocking(move || validador.validar(&caminho, &publicador))
                .await
                .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;
        assinatura.map_err(|falha| {
            ErroInstalacao::AssinaturaInvalida(match falha {
                FalhaDeAssinatura::NaoAssinado => "o arquivo não é assinado".to_owned(),
                FalhaDeAssinatura::Invalida(status) => format!("assinatura {status}"),
                FalhaDeAssinatura::PublicadorDiferente(assunto) => {
                    format!("publicador inesperado: {assunto}")
                }
                FalhaDeAssinatura::NaoFoiPossivelVerificar(detalhe) => detalhe,
            })
        })?;

        if cancelamento.is_cancelled() {
            return Err(Interrupcao::Cancelado);
        }
        Ok(())
    }

    /// Bloqueia antes de baixar se não houver espaço. Se não for possível ler o espaço
    /// livre, não bloqueia: o instalador do Ollama desfaz sozinho se o disco encher.
    fn verificar_espaco(&self, tamanho_do_instalador: u64) -> Result<(), ErroInstalacao> {
        let pasta_download = &self.config.pasta_temporaria;
        let pasta_instalacao = &self.config.pasta_de_instalacao;

        let exigencias: Vec<(&Path, u64)> = if mesma_unidade(pasta_download, pasta_instalacao) {
            vec![(
                pasta_download,
                tamanho_do_instalador.saturating_add(ESPACO_PARA_INSTALAR_EM_BYTES),
            )]
        } else {
            vec![
                (pasta_download, tamanho_do_instalador),
                (pasta_instalacao, ESPACO_PARA_INSTALAR_EM_BYTES),
            ]
        };

        for (pasta, necessario) in exigencias {
            if let Ok(livre) = self.disco.livre_em(pasta)
                && livre < necessario
            {
                return Err(ErroInstalacao::SemEspaco {
                    necessario_em_bytes: necessario,
                    disponivel_em_bytes: livre,
                });
            }
        }
        Ok(())
    }

    /// Rollback de uma instalação parcial. Devolve `true` se sobraram restos.
    ///
    /// Só mexe na pasta de instalação se ela **não existia antes** da tentativa: nunca
    /// apaga algo que já era do usuário. Os dados dos modelos ficam fora dela e não são
    /// tocados.
    async fn desfazer_instalacao_parcial(&self, existia_antes: bool) -> bool {
        let pasta = &self.config.pasta_de_instalacao;
        if existia_antes {
            // Não é nossa para limpar. Só reporta se o Ollama chegou a ser instalado.
            return self.detector.encontrar().is_some();
        }
        if !pasta.exists() {
            return false;
        }

        let desinstalador = pasta.join("unins000.exe");
        if desinstalador.is_file() {
            let executor = Arc::clone(&self.executor);
            let _ = tokio::task::spawn_blocking(move || executor.desinstalar(&desinstalador)).await;
        }
        if pasta.exists() {
            let _ = tokio::fs::remove_dir_all(pasta).await;
        }
        pasta.exists()
    }
}
