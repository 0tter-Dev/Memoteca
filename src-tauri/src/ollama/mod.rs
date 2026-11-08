//! Cliente da API HTTP do Ollama (E05), conforme o "Contrato Ollama" do `PLANO.md`.
//!
//! Só o backend Rust fala com o Ollama. O cliente consulta a versão, lista modelos, baixa
//! modelos com progresso e conversa com streaming, separando `content` de `thinking`.
//! Toda falha vira um `ErroOllama` tipado, e o texto que já tinha chegado de uma conversa
//! é preservado em `FalhaChat`.

pub mod ciclo;
pub mod erro;
pub mod instalacao;
#[cfg(windows)]
mod job;
mod ndjson;
pub mod processo;
pub mod tipos;
pub mod versao;

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

pub use erro::{ErroOllama, FalhaChat, TextoParcial};
pub use tipos::{
    DetalhesModelo, InfoVersao, KeepAlive, MensagemPedido, ModeloInstalado, MotivoFim, PapelChat,
    PedacoChat, PedidoChat, ProgressoDownload, RespostaChat, ResultadoDownload,
};
pub use versao::{VERSAO_MINIMA, Versao};

use ndjson::DivisorDeLinhas;
use tipos::{CorpoErro, LinhaChat, LinhaPull, RespostaShow, RespostaTags, RespostaVersao};

/// Endereço fixo do Ollama local. O IP literal evita falhas de resolução de `localhost`.
pub const ENDERECO_PADRAO: &str = "http://127.0.0.1:11434";

/// Limites de tempo do cliente. Os valores padrão são os do contrato e podem ser ajustados.
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    /// Tempo máximo para estabelecer a conexão. No Windows, conectar a uma porta onde
    /// ninguém ouve leva cerca de 2 s até o sistema recusar, então o padrão é maior que isso.
    /// Se o tempo estourar durante a conexão, o erro é `OllamaForaDoAr`.
    pub conexao: Duration,
    /// Tempo total máximo de `/api/version`, `/api/tags` e `/api/show`.
    pub consulta: Duration,
    /// Tempo máximo sem receber nenhum byte durante uma conversa (inclui a espera pelo
    /// primeiro byte, que cobre o carregamento do modelo).
    pub ociosidade_chat: Duration,
    /// Tempo máximo sem receber nenhum byte durante um download.
    pub ociosidade_pull: Duration,
}

impl Default for Limites {
    fn default() -> Self {
        Self {
            conexao: Duration::from_secs(3),
            consulta: Duration::from_secs(5),
            ociosidade_chat: Duration::from_secs(120),
            ociosidade_pull: Duration::from_secs(60),
        }
    }
}

pub struct ClienteOllama {
    http: reqwest::Client,
    base: String,
    limites: Limites,
}

/// Instrução devolvida por quem interpreta uma linha do stream.
enum Controle {
    Continuar,
    Terminar,
}

/// Como a leitura de um stream terminou.
enum FimDoFluxo {
    /// Quem interpretava as linhas pediu para parar (por exemplo, `done: true`).
    Terminou,
    Cancelado,
    /// A conexão acabou sem que o fim esperado tivesse chegado.
    Acabou,
}

impl ClienteOllama {
    /// Cliente para o Ollama local, em `127.0.0.1:11434`.
    pub fn novo() -> Result<Self, ErroOllama> {
        Self::com_endereco(ENDERECO_PADRAO, Limites::default())
    }

    /// Cliente para outro endereço e outros limites. Os testes usam isto para apontar
    /// para o servidor falso.
    pub fn com_endereco(base: &str, limites: Limites) -> Result<Self, ErroOllama> {
        let http = reqwest::Client::builder()
            // Tráfego para a própria máquina nunca deve passar por proxy.
            .no_proxy()
            .connect_timeout(limites.conexao)
            .build()
            .map_err(|erro| ErroOllama::ErroDoOllama(erro.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            limites,
        })
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}{caminho}", self.base)
    }

    // ----- Consultas simples -----

    /// `GET /api/version`. Confirma que quem responde na porta é mesmo um Ollama e informa
    /// a versão. Versão abaixo da mínima **não é erro**: veja `InfoVersao::aviso`.
    pub async fn verificar(&self) -> Result<InfoVersao, ErroOllama> {
        let resposta = self
            .http
            .get(self.url("/api/version"))
            .timeout(self.limites.consulta)
            .send()
            .await
            .map_err(|erro| {
                if erro.is_connect() {
                    ErroOllama::OllamaForaDoAr
                } else {
                    // Conectou, mas quem respondeu não falou como um Ollama.
                    ErroOllama::PortaOcupada
                }
            })?;

        if !resposta.status().is_success() {
            return Err(ErroOllama::PortaOcupada);
        }
        let bytes = resposta
            .bytes()
            .await
            .map_err(|_| ErroOllama::PortaOcupada)?;
        let corpo: RespostaVersao =
            serde_json::from_slice(&bytes).map_err(|_| ErroOllama::PortaOcupada)?;
        let versao = Versao::ler(&corpo.version).ok_or(ErroOllama::PortaOcupada)?;

        Ok(InfoVersao {
            versao,
            atende_o_minimo: versao >= VERSAO_MINIMA,
        })
    }

    /// `GET /api/tags`: modelos instalados.
    pub async fn listar_modelos(&self) -> Result<Vec<ModeloInstalado>, ErroOllama> {
        let corpo: RespostaTags = self.consultar(self.http.get(self.url("/api/tags"))).await?;

        Ok(corpo
            .models
            .into_iter()
            .map(|modelo| {
                let detalhes = modelo.details;
                ModeloInstalado {
                    nome: modelo.name,
                    tamanho_em_bytes: modelo.size,
                    familia: detalhes.as_ref().and_then(|d| d.family.clone()),
                    tamanho_dos_parametros: detalhes
                        .as_ref()
                        .and_then(|d| d.parameter_size.clone()),
                    quantizacao: detalhes.and_then(|d| d.quantization_level),
                }
            })
            .collect())
    }

    /// `POST /api/show`: capacidades e janela de contexto de um modelo.
    pub async fn detalhes_do_modelo(&self, nome: &str) -> Result<DetalhesModelo, ErroOllama> {
        let corpo: RespostaShow = self
            .consultar(
                self.http
                    .post(self.url("/api/show"))
                    .header(CONTENT_TYPE, "application/json")
                    .body(json!({ "model": nome }).to_string()),
            )
            .await?;

        let janela_de_contexto = corpo.model_info.as_ref().and_then(|info| {
            info.iter()
                .find(|(chave, _)| chave.ends_with(".context_length"))
                .and_then(|(_, valor)| valor.as_u64())
        });

        Ok(DetalhesModelo {
            capacidades: corpo.capabilities,
            janela_de_contexto,
            tem_metadados_de_raciocinio: corpo.thinking.is_some(),
        })
    }

    // ----- Download de modelo -----

    /// `POST /api/pull` com progresso. Cancelar fecha a conexão. O Ollama retoma depois um
    /// download cancelado do ponto onde parou.
    pub async fn baixar_modelo(
        &self,
        nome: &str,
        cancelamento: &CancellationToken,
        ao_progredir: &mut (dyn FnMut(ProgressoDownload) + Send),
    ) -> Result<ResultadoDownload, ErroOllama> {
        let corpo = json!({ "model": nome, "stream": true });
        let Some(resposta) = self
            .abrir_stream(
                "/api/pull",
                &corpo,
                self.limites.ociosidade_pull,
                cancelamento,
            )
            .await?
        else {
            return Ok(ResultadoDownload::Cancelado);
        };

        let mut acumulador = AcumuladorDeProgresso::default();
        let mut concluiu = false;
        let fim = ler_linhas(
            resposta,
            self.limites.ociosidade_pull,
            cancelamento,
            |linha| {
                let dados: LinhaPull = serde_json::from_slice(linha)
                    .map_err(|erro| ErroOllama::RespostaInvalida(erro.to_string()))?;
                if let Some(detalhe) = dados.error {
                    return Err(ErroOllama::ErroDoOllama(detalhe));
                }

                let fase = dados.status.unwrap_or_default();
                acumulador.registrar(dados.digest, dados.total, dados.completed);
                ao_progredir(acumulador.progresso(&fase));

                if fase == "success" {
                    concluiu = true;
                    return Ok(Controle::Terminar);
                }
                Ok(Controle::Continuar)
            },
        )
        .await?;

        match fim {
            FimDoFluxo::Cancelado => Ok(ResultadoDownload::Cancelado),
            _ if concluiu => Ok(ResultadoDownload::Concluido),
            _ => Err(ErroOllama::ConexaoInterrompida),
        }
    }

    // ----- Conversa -----

    /// `POST /api/chat` com streaming. `ao_receber` recebe cada pedaço na ordem em que
    /// chega, com o raciocínio separado do conteúdo.
    ///
    /// * Cancelar (`cancelamento`) fecha a conexão e devolve `Ok` com `MotivoFim::Cancelada`
    ///   e o texto recebido até ali. Depois do cancelamento, `ao_receber` não é chamado.
    /// * Qualquer falha devolve `FalhaChat`, com o texto parcial já recebido preservado.
    pub async fn conversar(
        &self,
        pedido: &PedidoChat,
        cancelamento: &CancellationToken,
        ao_receber: &mut (dyn FnMut(PedacoChat) + Send),
    ) -> Result<RespostaChat, FalhaChat> {
        let mut texto = TextoParcial::default();

        let corpo = pedido.para_json();
        let resposta = match self
            .abrir_stream(
                "/api/chat",
                &corpo,
                self.limites.ociosidade_chat,
                cancelamento,
            )
            .await
        {
            Ok(Some(resposta)) => resposta,
            Ok(None) => {
                return Ok(RespostaChat {
                    texto,
                    fim: MotivoFim::Cancelada,
                });
            }
            Err(erro) => {
                return Err(FalhaChat {
                    erro,
                    parcial: texto,
                });
            }
        };

        let mut motivo: Option<String> = None;
        let resultado = ler_linhas(
            resposta,
            self.limites.ociosidade_chat,
            cancelamento,
            |linha| {
                let dados: LinhaChat = serde_json::from_slice(linha)
                    .map_err(|erro| ErroOllama::RespostaInvalida(erro.to_string()))?;
                if let Some(detalhe) = dados.error {
                    return Err(ErroOllama::ErroDoOllama(detalhe));
                }

                if let Some(mensagem) = dados.message {
                    if let Some(pensamento) = mensagem.thinking.filter(|t| !t.is_empty()) {
                        texto
                            .raciocinio
                            .get_or_insert_with(String::new)
                            .push_str(&pensamento);
                        ao_receber(PedacoChat::Raciocinio(pensamento));
                    }
                    if let Some(conteudo) = mensagem.content.filter(|c| !c.is_empty()) {
                        texto.conteudo.push_str(&conteudo);
                        ao_receber(PedacoChat::Conteudo(conteudo));
                    }
                }

                if dados.done == Some(true) {
                    motivo = dados.done_reason;
                    return Ok(Controle::Terminar);
                }
                Ok(Controle::Continuar)
            },
        )
        .await;

        match resultado {
            Ok(FimDoFluxo::Terminou) => Ok(RespostaChat {
                texto,
                fim: MotivoFim::Concluida { motivo },
            }),
            Ok(FimDoFluxo::Cancelado) => Ok(RespostaChat {
                texto,
                fim: MotivoFim::Cancelada,
            }),
            Ok(FimDoFluxo::Acabou) => Err(FalhaChat {
                erro: ErroOllama::ConexaoInterrompida,
                parcial: texto,
            }),
            Err(erro) => Err(FalhaChat {
                erro,
                parcial: texto,
            }),
        }
    }

    // ----- Apoio -----

    /// Envia uma consulta curta e lê a resposta JSON inteira.
    async fn consultar<T: DeserializeOwned>(
        &self,
        requisicao: reqwest::RequestBuilder,
    ) -> Result<T, ErroOllama> {
        let resposta = requisicao
            .timeout(self.limites.consulta)
            .send()
            .await
            .map_err(|erro| erro_de_transporte(&erro))?;
        if !resposta.status().is_success() {
            return Err(erro_de_status(resposta).await);
        }
        let bytes = resposta
            .bytes()
            .await
            .map_err(|erro| erro_de_transporte(&erro))?;
        serde_json::from_slice(&bytes)
            .map_err(|erro| ErroOllama::RespostaInvalida(erro.to_string()))
    }

    /// Abre um `POST` em stream. `Ok(None)` significa que foi cancelado antes da resposta.
    async fn abrir_stream(
        &self,
        caminho: &str,
        corpo: &Value,
        ociosidade: Duration,
        cancelamento: &CancellationToken,
    ) -> Result<Option<reqwest::Response>, ErroOllama> {
        let envio = self
            .http
            .post(self.url(caminho))
            .header(CONTENT_TYPE, "application/json")
            .body(corpo.to_string())
            .send();

        let resposta = tokio::select! {
            biased;
            () = cancelamento.cancelled() => return Ok(None),
            resultado = tokio::time::timeout(ociosidade, envio) => resultado,
        };
        let resposta = match resposta {
            Err(_) => return Err(ErroOllama::TempoEsgotado),
            Ok(Err(erro)) => return Err(erro_de_transporte(&erro)),
            Ok(Ok(resposta)) => resposta,
        };

        if !resposta.status().is_success() {
            return Err(erro_de_status(resposta).await);
        }
        Ok(Some(resposta))
    }
}

/// Lê um stream NDJSON e entrega cada linha completa a `tratar`.
///
/// Devolve `Cancelado` assim que o cancelamento é pedido, sem entregar mais nenhuma linha.
/// Sai da função (e com isso fecha a conexão) em qualquer fim.
async fn ler_linhas<F>(
    resposta: reqwest::Response,
    ociosidade: Duration,
    cancelamento: &CancellationToken,
    mut tratar: F,
) -> Result<FimDoFluxo, ErroOllama>
where
    F: FnMut(&[u8]) -> Result<Controle, ErroOllama>,
{
    let fluxo = resposta.bytes_stream();
    tokio::pin!(fluxo);
    let mut divisor = DivisorDeLinhas::novo();

    loop {
        let proximo = tokio::select! {
            biased;
            () = cancelamento.cancelled() => return Ok(FimDoFluxo::Cancelado),
            resultado = tokio::time::timeout(ociosidade, fluxo.next()) => resultado,
        };

        let bytes = match proximo {
            Err(_) => return Err(ErroOllama::TempoEsgotado),
            Ok(Some(Err(erro))) => return Err(erro_de_transporte(&erro)),
            Ok(Some(Ok(bytes))) => bytes,
            Ok(None) => {
                // Fim da conexão: a última linha pode ter chegado sem `\n`.
                if let Some(linha) = divisor.finalizar() {
                    if cancelamento.is_cancelled() {
                        return Ok(FimDoFluxo::Cancelado);
                    }
                    if let Controle::Terminar = tratar(&linha)? {
                        return Ok(FimDoFluxo::Terminou);
                    }
                }
                return Ok(FimDoFluxo::Acabou);
            }
        };

        let linhas = divisor.empurrar(&bytes).map_err(|_| {
            ErroOllama::RespostaInvalida("linha maior que o limite aceito".to_owned())
        })?;
        for linha in linhas {
            if cancelamento.is_cancelled() {
                return Ok(FimDoFluxo::Cancelado);
            }
            if let Controle::Terminar = tratar(&linha)? {
                return Ok(FimDoFluxo::Terminou);
            }
        }
    }
}

/// Traduz uma falha de rede para o erro tipado.
fn erro_de_transporte(erro: &reqwest::Error) -> ErroOllama {
    if erro.is_connect() {
        ErroOllama::OllamaForaDoAr
    } else if erro.is_timeout() {
        ErroOllama::TempoEsgotado
    } else {
        ErroOllama::ConexaoInterrompida
    }
}

/// Traduz uma resposta HTTP de erro. O texto do Ollama só entra no detalhe técnico.
async fn erro_de_status(resposta: reqwest::Response) -> ErroOllama {
    let status = resposta.status();
    let detalhe = match resposta.bytes().await {
        Ok(bytes) => serde_json::from_slice::<CorpoErro>(&bytes)
            .ok()
            .and_then(|corpo| corpo.error)
            .unwrap_or_default(),
        Err(_) => String::new(),
    };

    if status == reqwest::StatusCode::NOT_FOUND {
        ErroOllama::ModeloNaoEncontrado
    } else {
        ErroOllama::ErroDoOllama(format!("HTTP {status}: {detalhe}"))
    }
}

/// Soma o progresso de todas as camadas (`digest`) de um download.
#[derive(Default)]
struct AcumuladorDeProgresso {
    camadas: HashMap<String, (u64, u64)>,
}

impl AcumuladorDeProgresso {
    fn registrar(&mut self, digest: Option<String>, total: Option<u64>, baixado: Option<u64>) {
        let (Some(digest), Some(total)) = (digest, total) else {
            return;
        };
        let camada = self.camadas.entry(digest).or_default();
        camada.0 = total;
        if let Some(baixado) = baixado {
            camada.1 = baixado.min(total);
        }
    }

    fn progresso(&self, fase: &str) -> ProgressoDownload {
        let total: u64 = self.camadas.values().map(|(total, _)| total).sum();
        let baixado: u64 = self.camadas.values().map(|(_, baixado)| baixado).sum();
        ProgressoDownload {
            fase: fase.to_owned(),
            total_em_bytes: total,
            baixado_em_bytes: baixado,
            fracao: (total > 0).then(|| (baixado as f64 / total as f64).min(1.0)),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_progresso_soma_as_camadas_e_fica_indeterminado_sem_total() {
        let mut acumulador = AcumuladorDeProgresso::default();
        assert_eq!(acumulador.progresso("pulling manifest").fracao, None);

        acumulador.registrar(Some("a".into()), Some(1_000), None);
        acumulador.registrar(Some("b".into()), Some(3_000), Some(500));
        let progresso = acumulador.progresso("pulling a");
        assert_eq!(progresso.total_em_bytes, 4_000);
        assert_eq!(progresso.baixado_em_bytes, 500);
        assert_eq!(progresso.fracao, Some(0.125));

        acumulador.registrar(Some("a".into()), Some(1_000), Some(1_000));
        acumulador.registrar(Some("b".into()), Some(3_000), Some(3_000));
        assert_eq!(acumulador.progresso("success").fracao, Some(1.0));
    }

    #[test]
    fn linhas_sem_digest_ou_sem_total_nao_alteram_o_progresso() {
        let mut acumulador = AcumuladorDeProgresso::default();

        acumulador.registrar(None, Some(10), Some(5));
        acumulador.registrar(Some("a".into()), None, Some(5));

        assert_eq!(acumulador.progresso("x").total_em_bytes, 0);
    }

    #[test]
    fn o_baixado_nunca_passa_do_total() {
        let mut acumulador = AcumuladorDeProgresso::default();

        acumulador.registrar(Some("a".into()), Some(100), Some(250));

        assert_eq!(acumulador.progresso("x").fracao, Some(1.0));
    }
}
