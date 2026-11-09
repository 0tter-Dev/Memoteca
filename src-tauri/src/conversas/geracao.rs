//! Geração da resposta de uma conversa, com streaming e persistência.
//!
//! * **Responder** a última mensagem do usuário: a resposta vira uma nova mensagem.
//! * **Regenerar** a última resposta: a resposta nova **substitui** a antiga (sem histórico de
//!   versões). Se a geração falhar, a resposta antiga é mantida.
//!
//! A mensagem do usuário é gravada **antes** de chamar o Ollama, então nunca se perde. Uma
//! resposta que chegou pela metade (cancelada ou com a conexão caída) é guardada como
//! `interrompida`, para não perder o que já foi gerado.

use std::sync::Mutex;

use crate::banco::{Banco, ErroBanco, EstadoMensagem, Mensagem, Papel};
use crate::ollama::{
    ClienteOllama, ErroOllama, FalhaChat, KeepAlive, MotivoFim, PedacoChat, PedidoChat,
    TextoParcial,
};
use tokio_util::sync::CancellationToken;

use super::contexto::montar_contexto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoDeGeracao {
    /// A última mensagem é do usuário e ainda não tem resposta.
    ResponderUltimaMensagem,
    /// A última mensagem é uma resposta, que será substituída pela nova.
    RegenerarUltimaResposta,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErroGeracao {
    /// Já há uma geração em andamento (uma conversa ativa por vez).
    JaGerando,
    /// A conversa ainda não tem um modelo escolhido.
    SemModelo,
    /// Não há mensagem do usuário sem resposta.
    NadaParaResponder,
    /// Não há resposta para regenerar.
    NadaParaRegenerar,
    /// O modelo terminou sem devolver texto.
    RespostaVazia,
    Banco(ErroBanco),
    Ollama(ErroOllama),
}

impl ErroGeracao {
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::JaGerando => {
                "Já há uma resposta sendo gerada. Espere terminar ou cancele.".to_owned()
            }
            Self::SemModelo => "Escolha um modelo para esta conversa antes de enviar.".to_owned(),
            Self::NadaParaResponder => "Não há mensagem para responder.".to_owned(),
            Self::NadaParaRegenerar => "Não há resposta para gerar de novo.".to_owned(),
            Self::RespostaVazia => {
                "O modelo não devolveu nenhuma resposta. Tente de novo.".to_owned()
            }
            Self::Banco(erro) => erro.mensagem_amigavel(),
            Self::Ollama(erro) => erro.mensagem_amigavel(),
        }
    }
}

impl std::fmt::Display for ErroGeracao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JaGerando => write!(f, "já há uma geração em andamento"),
            Self::SemModelo => write!(f, "a conversa não tem modelo"),
            Self::NadaParaResponder => write!(f, "não há mensagem do usuário sem resposta"),
            Self::NadaParaRegenerar => write!(f, "não há resposta para regenerar"),
            Self::RespostaVazia => write!(f, "resposta vazia"),
            Self::Banco(erro) => write!(f, "{erro}"),
            Self::Ollama(erro) => write!(f, "{erro}"),
        }
    }
}

impl std::error::Error for ErroGeracao {}

impl From<ErroBanco> for ErroGeracao {
    fn from(erro: ErroBanco) -> Self {
        Self::Banco(erro)
    }
}

/// Falha de uma geração. A mensagem do usuário continua gravada, e `mensagem_parcial` traz a
/// resposta que chegou pela metade e foi guardada, se houver.
#[derive(Debug, Clone, PartialEq)]
pub struct FalhaDeGeracao {
    pub erro: ErroGeracao,
    pub mensagem_parcial: Option<Box<Mensagem>>,
}

impl From<ErroGeracao> for FalhaDeGeracao {
    fn from(erro: ErroGeracao) -> Self {
        Self {
            erro,
            mensagem_parcial: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesfechoDaGeracao {
    /// A resposta terminou e foi gravada. \`cortada\`: o modelo parou por limite de tamanho.
    Concluida { mensagem: Mensagem, cortada: bool },
    /// O usuário cancelou. \`mensagem\` traz o que já tinha chegado, gravado como interrompido.
    Cancelada { mensagem: Option<Mensagem> },
}

/// Roda uma operação no banco, sem segurar o bloqueio entre um \`await\` e outro.
fn com_banco<T>(
    banco: &Mutex<Banco>,
    operacao: impl FnOnce(&Banco) -> Result<T, ErroBanco>,
) -> Result<T, ErroBanco> {
    let banco = banco
        .lock()
        .map_err(|_| ErroBanco::Interno("banco em uso por outra operação".into()))?;
    operacao(&banco)
}

fn keep_alive_do_ollama(texto: &str) -> KeepAlive {
    match texto {
        "-1" => KeepAlive::Segundos(-1),
        "0" => KeepAlive::Segundos(0),
        outro => KeepAlive::Duracao(outro.to_owned()),
    }
}

fn texto_ou_nada(texto: String) -> Option<String> {
    (!texto.is_empty()).then_some(texto)
}

/// Gera a resposta da conversa. \`ao_pedaco\` recebe, na ordem, o raciocínio e o conteúdo que
/// chegam do modelo.
pub async fn gerar(
    banco: &Mutex<Banco>,
    cliente: &ClienteOllama,
    conversa_id: &str,
    modo: ModoDeGeracao,
    cancelamento: &CancellationToken,
    ao_pedaco: &mut (dyn FnMut(PedacoChat) + Send),
) -> Result<DesfechoDaGeracao, FalhaDeGeracao> {
    // 1. O que precisa ser lido do banco, de uma vez.
    let (conversa, instrucoes, mensagens) = com_banco(banco, |b| {
        let conversa = b.obter_conversa(conversa_id)?;
        let projeto = b.obter_projeto(&conversa.projeto_id)?;
        let mensagens = b.listar_mensagens(conversa_id)?;
        Ok((conversa, projeto.system_prompt, mensagens))
    })
    .map_err(ErroGeracao::from)?;

    let modelo = conversa
        .modelo
        .clone()
        .filter(|modelo| !modelo.trim().is_empty())
        .ok_or(ErroGeracao::SemModelo)?;

    let ultima = mensagens.last();
    let (historico, alvo) = match modo {
        ModoDeGeracao::ResponderUltimaMensagem => match ultima {
            Some(m) if m.papel == Papel::Usuario => (&mensagens[..], None),
            _ => return Err(ErroGeracao::NadaParaResponder.into()),
        },
        ModoDeGeracao::RegenerarUltimaResposta => match ultima {
            Some(m) if m.papel == Papel::Assistente => {
                (&mensagens[..mensagens.len() - 1], Some(m.id.clone()))
            }
            _ => return Err(ErroGeracao::NadaParaRegenerar.into()),
        },
    };

    // 2. O modelo: precisa existir, servir para conversa e informar a janela de contexto, que
    // limita o `num_ctx` pedido.
    let janela = match cliente.detalhes_do_modelo(&modelo).await {
        Ok(detalhes) => {
            detalhes.exige_conversa().map_err(ErroGeracao::Ollama)?;
            detalhes.janela_de_contexto
        }
        Err(
            erro @ (ErroOllama::ModeloNaoEncontrado
            | ErroOllama::OllamaForaDoAr
            | ErroOllama::PortaOcupada),
        ) => return Err(ErroGeracao::Ollama(erro).into()),
        // Qualquer outra falha ao consultar os detalhes não impede de conversar.
        Err(_) => None,
    };
    let num_ctx = conversa.num_ctx.map(|pedido| {
        let limitado = janela.map_or(pedido, |janela| {
            pedido.min(i64::try_from(janela).unwrap_or(i64::MAX))
        });
        u32::try_from(limitado).unwrap_or(u32::MAX)
    });

    let pedido = PedidoChat {
        modelo,
        mensagens: montar_contexto(&instrucoes, historico),
        temperature: conversa.temperature,
        num_ctx,
        keep_alive: conversa.keep_alive.as_deref().map(keep_alive_do_ollama),
    };

    // 3. A geração, com streaming.
    let resultado = cliente.conversar(&pedido, cancelamento, ao_pedaco).await;

    // 4. O que fica gravado.
    let gravar = |conteudo: &str,
                  raciocinio: Option<&str>,
                  estado: EstadoMensagem|
     -> Result<Mensagem, ErroBanco> {
        com_banco(banco, |b| {
            let mensagem = match &alvo {
                Some(id) => b.substituir_mensagem_com_estado(id, conteudo, raciocinio, estado)?,
                None => b.adicionar_mensagem_com_estado(
                    conversa_id,
                    Papel::Assistente,
                    conteudo,
                    raciocinio,
                    estado,
                )?,
            };
            b.tocar_conversa(conversa_id)?;
            Ok(mensagem)
        })
    };

    match resultado {
        Ok(resposta) => {
            let TextoParcial {
                conteudo,
                raciocinio,
            } = resposta.texto.clone();
            match resposta.fim {
                MotivoFim::Concluida { .. } => {
                    if conteudo.trim().is_empty() && raciocinio.as_deref().is_none_or(str::is_empty)
                    {
                        return Err(ErroGeracao::RespostaVazia.into());
                    }
                    let mensagem =
                        gravar(&conteudo, raciocinio.as_deref(), EstadoMensagem::Completa)
                            .map_err(ErroGeracao::from)?;
                    Ok(DesfechoDaGeracao::Concluida {
                        mensagem,
                        cortada: resposta.foi_cortada(),
                    })
                }
                MotivoFim::Cancelada => {
                    // Cancelou com texto: guarda o que chegou. Sem texto, não muda nada (ao
                    // regenerar, a resposta antiga continua).
                    let mensagem = if conteudo.trim().is_empty() {
                        None
                    } else {
                        Some(
                            gravar(
                                &conteudo,
                                raciocinio.as_deref(),
                                EstadoMensagem::Interrompida,
                            )
                            .map_err(ErroGeracao::from)?,
                        )
                    };
                    Ok(DesfechoDaGeracao::Cancelada { mensagem })
                }
            }
        }
        Err(FalhaChat { erro, parcial }) => {
            // Falhou no meio. Ao responder, o que chegou é guardado como interrompido. Ao
            // regenerar, a resposta antiga é mantida e o parcial se perde.
            let mensagem_parcial = if alvo.is_none() && !parcial.conteudo.trim().is_empty() {
                gravar(
                    &parcial.conteudo,
                    parcial
                        .raciocinio
                        .as_deref()
                        .and_then(|r| texto_ou_nada(r.to_owned()))
                        .as_deref(),
                    EstadoMensagem::Interrompida,
                )
                .ok()
                .map(Box::new)
            } else {
                None
            };
            Err(FalhaDeGeracao {
                erro: ErroGeracao::Ollama(erro),
                mensagem_parcial,
            })
        }
    }
}
