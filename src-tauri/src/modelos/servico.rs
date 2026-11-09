//! Download guiado de modelos: valida o nome, aplica a regra de espaço, baixa com progresso
//! e confere que o modelo apareceu na lista do Ollama.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::instalador::disco::EspacoEmDisco;
use crate::ollama::{ClienteOllama, ErroOllama, ProgressoDownload, ResultadoDownload};

use super::catalogo::Catalogo;
use super::espaco::{self, AvaliacaoDeEspaco, mensagem_de_espaco_insuficiente};
use super::nome::{ErroNome, NomeDeModelo};

/// De quanto em quanto tempo o espaço livre é conferido durante o download.
const INTERVALO_DA_CONFERENCIA_DE_ESPACO: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroModelo {
    Nome(ErroNome),
    /// Não há espaço para o que falta baixar. Bloqueia ou interrompe o download.
    SemEspaco {
        necessario_em_bytes: u64,
        disponivel_em_bytes: u64,
    },
    Ollama(ErroOllama),
    /// O download terminou, mas o modelo não aparece na lista do Ollama.
    NaoApareceuNaLista,
}

impl ErroModelo {
    /// Mensagem em pt-BR para mostrar ao usuário.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::Nome(erro) => erro.mensagem_amigavel(),
            Self::SemEspaco {
                necessario_em_bytes,
                disponivel_em_bytes,
            } => mensagem_de_espaco_insuficiente(*necessario_em_bytes, *disponivel_em_bytes),
            Self::Ollama(ErroOllama::ConexaoInterrompida | ErroOllama::TempoEsgotado) => {
                "O download foi interrompido, mas o que já foi baixado foi guardado. Clique em \
                 Continuar para retomar de onde parou."
                    .to_owned()
            }
            Self::Ollama(erro) => erro.mensagem_amigavel(),
            Self::NaoApareceuNaLista => "O download terminou, mas o modelo não apareceu na \
                lista do Ollama. Tente baixar de novo."
                .to_owned(),
        }
    }

    /// Tentar de novo retoma do ponto onde parou (o botão diz "Continuar").
    pub fn pode_continuar(&self) -> bool {
        matches!(
            self,
            Self::Ollama(ErroOllama::ConexaoInterrompida | ErroOllama::TempoEsgotado)
        )
    }
}

impl std::fmt::Display for ErroModelo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nome(erro) => write!(f, "{erro}"),
            Self::SemEspaco {
                necessario_em_bytes,
                disponivel_em_bytes,
            } => write!(
                f,
                "sem espaço: faltam {necessario_em_bytes} bytes e há {disponivel_em_bytes}"
            ),
            Self::Ollama(erro) => write!(f, "{erro}"),
            Self::NaoApareceuNaLista => write!(f, "o modelo não apareceu na lista do Ollama"),
        }
    }
}

impl std::error::Error for ErroModelo {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaseDoDownload {
    Preparando,
    Baixando,
    Verificando,
    Finalizando,
    Concluido,
}

impl FaseDoDownload {
    /// Traduz o texto de fase do Ollama (`pulling manifest`, `pulling <digest>`,
    /// `verifying sha256 digest`...) para uma fase que a tela entende.
    fn do_ollama(fase: &str) -> Self {
        if fase == "success" {
            Self::Concluido
        } else if fase.starts_with("pulling ") && fase != "pulling manifest" {
            Self::Baixando
        } else if fase.starts_with("verifying") {
            Self::Verificando
        } else if fase.starts_with("writing") || fase.starts_with("removing") {
            Self::Finalizando
        } else {
            Self::Preparando
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressoDoModelo {
    pub fase: FaseDoDownload,
    pub baixado_em_bytes: u64,
    pub total_em_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoDoModelo {
    Concluido,
    Cancelado,
}

pub struct ServicoDeModelos {
    cliente: ClienteOllama,
    disco: Arc<dyn EspacoEmDisco>,
    pasta_de_modelos: Option<PathBuf>,
    catalogo: Catalogo,
}

impl ServicoDeModelos {
    pub fn novo(
        cliente: ClienteOllama,
        disco: Arc<dyn EspacoEmDisco>,
        pasta_de_modelos: Option<PathBuf>,
        catalogo: Catalogo,
    ) -> Self {
        Self {
            cliente,
            disco,
            pasta_de_modelos,
            catalogo,
        }
    }

    pub fn catalogo(&self) -> &Catalogo {
        &self.catalogo
    }

    /// Janela de contexto e suporte a raciocínio de um modelo instalado.
    pub async fn detalhes(&self, nome: &str) -> Result<crate::ollama::DetalhesModelo, ErroOllama> {
        self.cliente.detalhes_do_modelo(nome).await
    }

    /// Nomes dos modelos instalados no Ollama, como ele os lista.
    pub async fn listar_instalados(&self) -> Result<Vec<String>, ErroOllama> {
        Ok(self
            .cliente
            .listar_modelos()
            .await?
            .into_iter()
            .map(|modelo| modelo.nome)
            .collect())
    }

    fn espaco_livre(&self) -> Option<u64> {
        self.pasta_de_modelos
            .as_ref()
            .and_then(|pasta| self.disco.livre_em(pasta).ok())
    }

    /// Avalia o espaço para um modelo **do catálogo**, pelo tamanho conhecido. Para um modelo
    /// fora do catálogo o tamanho só é conhecido durante o download, e a avaliação é
    /// `Desconhecido`.
    pub fn avaliar_espaco(&self, tag: &str) -> (AvaliacaoDeEspaco, u64, Option<u64>) {
        let livre = self.espaco_livre();
        match self.catalogo.buscar(tag) {
            Some(modelo) => (
                espaco::avaliar(modelo.tamanho_em_bytes, livre),
                modelo.tamanho_em_bytes,
                livre,
            ),
            None => (AvaliacaoDeEspaco::Desconhecido, 0, livre),
        }
    }

    /// Baixa o modelo e confere que ele apareceu na lista do Ollama.
    ///
    /// * Um modelo do catálogo sem espaço é **bloqueado antes de começar**. Com pouca folga,
    ///   o download segue (a tela já pediu a confirmação do usuário).
    /// * Durante o download, o espaço que falta é conferido de tempos em tempos. Se deixar de
    ///   caber, o download é interrompido com `SemEspaco`. Isso cobre também os modelos
    ///   digitados à mão, cujo tamanho só aparece depois de o download começar.
    pub async fn baixar(
        &self,
        nome: &str,
        cancelamento: &CancellationToken,
        ao_progredir: &mut (dyn FnMut(ProgressoDoModelo) + Send),
    ) -> Result<ResultadoDoModelo, ErroModelo> {
        let nome = NomeDeModelo::validar(nome).map_err(ErroModelo::Nome)?;

        if let (
            AvaliacaoDeEspaco::Insuficiente {
                necessario_em_bytes,
                disponivel_em_bytes,
            },
            _,
            _,
        ) = self.avaliar_espaco(nome.como_texto())
        {
            return Err(ErroModelo::SemEspaco {
                necessario_em_bytes,
                disponivel_em_bytes,
            });
        }

        let interno = cancelamento.child_token();
        let mut sem_espaco: Option<(u64, u64)> = None;
        let mut ultima_conferencia: Option<Instant> = None;

        let resultado = self
            .cliente
            .baixar_modelo(
                nome.como_texto(),
                &interno,
                &mut |progresso: ProgressoDownload| {
                    ao_progredir(ProgressoDoModelo {
                        fase: FaseDoDownload::do_ollama(&progresso.fase),
                        baixado_em_bytes: progresso.baixado_em_bytes,
                        total_em_bytes: progresso.total_em_bytes,
                    });

                    // Confere se o que falta baixar ainda cabe, sem exagerar nas consultas.
                    let falta = progresso
                        .total_em_bytes
                        .saturating_sub(progresso.baixado_em_bytes);
                    let hora_de_conferir = ultima_conferencia.is_none_or(|ultima| {
                        ultima.elapsed() >= INTERVALO_DA_CONFERENCIA_DE_ESPACO
                    });
                    if falta > 0 && hora_de_conferir && sem_espaco.is_none() {
                        ultima_conferencia = Some(Instant::now());
                        if let Some(livre) = self.espaco_livre()
                            && livre < falta
                        {
                            sem_espaco = Some((falta, livre));
                            interno.cancel();
                        }
                    }
                },
            )
            .await
            .map_err(ErroModelo::Ollama)?;

        match resultado {
            ResultadoDownload::Cancelado => match sem_espaco {
                Some((necessario_em_bytes, disponivel_em_bytes)) => Err(ErroModelo::SemEspaco {
                    necessario_em_bytes,
                    disponivel_em_bytes,
                }),
                None => Ok(ResultadoDoModelo::Cancelado),
            },
            ResultadoDownload::Concluido => {
                let esperado = nome.com_tag_padrao().to_lowercase();
                let instalados = self
                    .cliente
                    .listar_modelos()
                    .await
                    .map_err(ErroModelo::Ollama)?;
                if instalados
                    .iter()
                    .any(|modelo| modelo.nome.to_lowercase() == esperado)
                {
                    Ok(ResultadoDoModelo::Concluido)
                } else {
                    Err(ErroModelo::NaoApareceuNaLista)
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn traduz_as_fases_do_ollama() {
        assert_eq!(
            FaseDoDownload::do_ollama("pulling manifest"),
            FaseDoDownload::Preparando
        );
        assert_eq!(
            FaseDoDownload::do_ollama("pulling 74701a8c35f6"),
            FaseDoDownload::Baixando
        );
        assert_eq!(
            FaseDoDownload::do_ollama("verifying sha256 digest"),
            FaseDoDownload::Verificando
        );
        assert_eq!(
            FaseDoDownload::do_ollama("writing manifest"),
            FaseDoDownload::Finalizando
        );
        assert_eq!(
            FaseDoDownload::do_ollama("removing any unused layers"),
            FaseDoDownload::Finalizando
        );
        assert_eq!(
            FaseDoDownload::do_ollama("success"),
            FaseDoDownload::Concluido
        );
        assert_eq!(
            FaseDoDownload::do_ollama("qualquer coisa"),
            FaseDoDownload::Preparando
        );
    }

    #[test]
    fn so_a_interrupcao_oferece_continuar() {
        assert!(ErroModelo::Ollama(ErroOllama::ConexaoInterrompida).pode_continuar());
        assert!(ErroModelo::Ollama(ErroOllama::TempoEsgotado).pode_continuar());
        assert!(!ErroModelo::Ollama(ErroOllama::ModeloInexistente).pode_continuar());
        assert!(!ErroModelo::NaoApareceuNaLista.pode_continuar());
        assert!(
            !ErroModelo::SemEspaco {
                necessario_em_bytes: 1,
                disponivel_em_bytes: 0
            }
            .pode_continuar()
        );
    }

    #[test]
    fn a_mensagem_da_interrupcao_explica_que_o_progresso_foi_guardado() {
        let mensagem = ErroModelo::Ollama(ErroOllama::ConexaoInterrompida).mensagem_amigavel();

        assert!(mensagem.contains("guardado") && mensagem.contains("Continuar"));
    }
}
