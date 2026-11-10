//! Tipos públicos do cliente e formatos de rede (internos) do Ollama.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::erro::{ErroOllama, TextoParcial};
use super::versao::{VERSAO_MINIMA, Versao};

// ----- Tipos públicos -----

/// Resultado da verificação do Ollama: ele existe, é um Ollama e tem esta versão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoVersao {
    pub versao: Versao,
    pub atende_o_minimo: bool,
}

impl InfoVersao {
    /// Aviso amigável quando a versão está abaixo da mínima. Não bloqueia o uso.
    pub fn aviso(&self) -> Option<ErroOllama> {
        (!self.atende_o_minimo).then_some(ErroOllama::VersaoAntiga {
            encontrada: self.versao,
            minima: VERSAO_MINIMA,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloInstalado {
    /// Nome como o Ollama devolve (`modelo:tag`). É o que se envia de volta nas chamadas.
    pub nome: String,
    pub tamanho_em_bytes: Option<u64>,
    pub familia: Option<String>,
    pub tamanho_dos_parametros: Option<String>,
    pub quantizacao: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalhesModelo {
    /// `capabilities` do `/api/show`. `None` quando a versão do Ollama não informa.
    pub capacidades: Option<Vec<String>>,
    /// Janela de contexto máxima do modelo, para limitar o `num_ctx` oferecido.
    pub janela_de_contexto: Option<u64>,
    /// O `/api/show` trouxe metadados de raciocínio (`thinking`).
    pub tem_metadados_de_raciocinio: bool,
}

impl DetalhesModelo {
    /// Sem a lista de capacidades não dá para afirmar que o modelo não conversa.
    pub fn suporta_conversa(&self) -> bool {
        self.capacidades
            .as_ref()
            .is_none_or(|lista| lista.iter().any(|c| c == "completion"))
    }

    pub fn suporta_raciocinio(&self) -> bool {
        self.tem_metadados_de_raciocinio
            || self
                .capacidades
                .as_ref()
                .is_some_and(|lista| lista.iter().any(|c| c == "thinking"))
    }

    /// `Err(ModeloSemSuporte)` se o modelo não serve para conversa.
    pub fn exige_conversa(&self) -> Result<(), ErroOllama> {
        if self.suporta_conversa() {
            Ok(())
        } else {
            Err(ErroOllama::ModeloSemSuporte)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapelChat {
    Sistema,
    Usuario,
    Assistente,
}

impl PapelChat {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Sistema => "system",
            Self::Usuario => "user",
            Self::Assistente => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MensagemPedido {
    pub papel: PapelChat,
    pub conteudo: String,
}

/// Quanto tempo o modelo fica na memória depois da resposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeepAlive {
    /// Texto de duração do Ollama, como `5m` ou `24h`.
    Duracao(String),
    /// Número de segundos. Negativo mantém carregado, e `0` descarrega logo.
    Segundos(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PedidoChat {
    pub modelo: String,
    pub mensagens: Vec<MensagemPedido>,
    pub temperature: Option<f64>,
    pub num_ctx: Option<u32>,
    pub keep_alive: Option<KeepAlive>,
}

impl PedidoChat {
    /// Corpo do `POST /api/chat`. Por decisão do contrato, `think` nunca é enviado, e as
    /// mensagens carregam só `role` e `content`.
    pub fn para_json(&self) -> Value {
        let mensagens: Vec<Value> = self
            .mensagens
            .iter()
            .map(|m| json!({ "role": m.papel.como_texto(), "content": m.conteudo }))
            .collect();

        let mut corpo = Map::new();
        corpo.insert("model".into(), json!(self.modelo));
        corpo.insert("messages".into(), Value::Array(mensagens));
        corpo.insert("stream".into(), json!(true));

        let mut opcoes = Map::new();
        if let Some(temperature) = self.temperature {
            opcoes.insert("temperature".into(), json!(temperature));
        }
        if let Some(num_ctx) = self.num_ctx {
            opcoes.insert("num_ctx".into(), json!(num_ctx));
        }
        if !opcoes.is_empty() {
            corpo.insert("options".into(), Value::Object(opcoes));
        }

        match &self.keep_alive {
            Some(KeepAlive::Duracao(texto)) => {
                corpo.insert("keep_alive".into(), json!(texto));
            }
            Some(KeepAlive::Segundos(segundos)) => {
                corpo.insert("keep_alive".into(), json!(segundos));
            }
            None => {}
        }
        Value::Object(corpo)
    }
}

/// Pedido de chat sem streaming, com a resposta restrita a um esquema JSON (`format`).
/// Serve para tarefas curtas em que o app precisa de uma resposta estruturada, como as
/// sugestões de memória. `think` vai como `false`: a tarefa não precisa de raciocínio, e
/// assim um modelo que raciocina não gasta o tempo todo pensando antes de responder.
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoEstruturado {
    pub modelo: String,
    pub mensagens: Vec<MensagemPedido>,
    /// JSON Schema da resposta.
    pub esquema: Value,
    pub temperature: Option<f64>,
    pub num_ctx: Option<u32>,
    pub keep_alive: Option<KeepAlive>,
}

impl PedidoEstruturado {
    pub fn para_json(&self) -> Value {
        // Reaproveita o corpo do chat comum e troca o que muda: sem stream, com `format` e
        // com `think: false`.
        let base = PedidoChat {
            modelo: self.modelo.clone(),
            mensagens: self.mensagens.clone(),
            temperature: self.temperature,
            num_ctx: self.num_ctx,
            keep_alive: self.keep_alive.clone(),
        };
        let mut corpo = base.para_json();
        if let Value::Object(mapa) = &mut corpo {
            mapa.insert("stream".into(), json!(false));
            mapa.insert("format".into(), self.esquema.clone());
            mapa.insert("think".into(), json!(false));
        }
        corpo
    }
}

/// Um pedaço de resposta, na ordem em que chegou. O raciocínio vem antes do conteúdo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedacoChat {
    Raciocinio(String),
    Conteudo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotivoFim {
    /// O Ollama terminou a resposta (`done: true`), com o `done_reason` que informou.
    Concluida { motivo: Option<String> },
    /// O usuário cancelou. O texto recebido até ali foi mantido.
    Cancelada,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespostaChat {
    pub texto: TextoParcial,
    pub fim: MotivoFim,
}

impl RespostaChat {
    /// A resposta foi cortada por limite de tamanho (`done_reason: "length"`).
    pub fn foi_cortada(&self) -> bool {
        matches!(&self.fim, MotivoFim::Concluida { motivo: Some(m) } if m == "length")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressoDownload {
    /// Fase informada pelo Ollama (`pulling manifest`, `verifying sha256 digest`...).
    pub fase: String,
    /// Total de bytes de todas as camadas já conhecidas.
    pub total_em_bytes: u64,
    pub baixado_em_bytes: u64,
    /// Entre 0 e 1. `None` enquanto o total ainda é desconhecido.
    pub fracao: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoDownload {
    Concluido,
    Cancelado,
}

// ----- Formatos de rede -----

#[derive(Deserialize)]
pub(super) struct RespostaVersao {
    pub version: String,
}

#[derive(Deserialize)]
pub(super) struct RespostaTags {
    pub models: Vec<ModeloBruto>,
}

#[derive(Deserialize)]
pub(super) struct ModeloBruto {
    pub name: String,
    pub size: Option<u64>,
    pub details: Option<DetalhesBrutos>,
}

#[derive(Deserialize)]
pub(super) struct DetalhesBrutos {
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RespostaShow {
    pub capabilities: Option<Vec<String>>,
    pub model_info: Option<Map<String, Value>>,
    pub thinking: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct LinhaChat {
    pub message: Option<MensagemBruta>,
    pub done: Option<bool>,
    pub done_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct MensagemBruta {
    pub content: Option<String>,
    pub thinking: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct LinhaPull {
    pub status: Option<String>,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CorpoErro {
    pub error: Option<String>,
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido() -> PedidoChat {
        PedidoChat {
            modelo: "qwen3:8b".into(),
            mensagens: vec![
                MensagemPedido {
                    papel: PapelChat::Sistema,
                    conteudo: "Seja breve.".into(),
                },
                MensagemPedido {
                    papel: PapelChat::Usuario,
                    conteudo: "Oi".into(),
                },
                MensagemPedido {
                    papel: PapelChat::Assistente,
                    conteudo: "Olá!".into(),
                },
            ],
            temperature: Some(0.7),
            num_ctx: Some(8192),
            keep_alive: Some(KeepAlive::Duracao("30m".into())),
        }
    }

    #[test]
    fn o_corpo_estruturado_nao_tem_stream_leva_o_esquema_e_desliga_o_raciocinio() {
        let esquema = json!({ "type": "object" });
        let estruturado = PedidoEstruturado {
            modelo: "qwen3:8b".into(),
            mensagens: pedido().mensagens,
            esquema: esquema.clone(),
            temperature: Some(0.2),
            num_ctx: Some(4096),
            keep_alive: None,
        };

        let corpo = estruturado.para_json();

        assert_eq!(corpo["stream"], false);
        assert_eq!(corpo["format"], esquema);
        assert_eq!(corpo["think"], false);
        assert_eq!(corpo["options"]["temperature"], 0.2);
        assert_eq!(corpo["options"]["num_ctx"], 4096);
        assert!(corpo.get("keep_alive").is_none());
        // As mensagens continuam só com `role` e `content`.
        let primeira = corpo["messages"][0].as_object().unwrap();
        assert_eq!(primeira.len(), 2);
    }

    #[test]
    fn o_corpo_do_chat_segue_o_contrato() {
        let corpo = pedido().para_json();

        assert_eq!(corpo["model"], "qwen3:8b");
        assert_eq!(corpo["stream"], true);
        assert_eq!(corpo["options"]["temperature"], 0.7);
        assert_eq!(corpo["options"]["num_ctx"], 8192);
        assert_eq!(corpo["keep_alive"], "30m");
        assert_eq!(corpo["messages"][0]["role"], "system");
        assert_eq!(corpo["messages"][1]["role"], "user");
        assert_eq!(corpo["messages"][2]["role"], "assistant");
    }

    #[test]
    fn o_corpo_nunca_envia_think_nem_campos_alem_de_role_e_content() {
        let corpo = pedido().para_json();

        assert!(corpo.get("think").is_none());
        for mensagem in corpo["messages"].as_array().unwrap() {
            let campos: Vec<&String> = mensagem.as_object().unwrap().keys().collect();
            assert_eq!(campos, ["content", "role"]);
        }
    }

    #[test]
    fn opcoes_vazias_nao_sao_enviadas() {
        let mut pedido = pedido();
        pedido.temperature = None;
        pedido.num_ctx = None;
        pedido.keep_alive = None;

        let corpo = pedido.para_json();

        assert!(corpo.get("options").is_none());
        assert!(corpo.get("keep_alive").is_none());
    }

    #[test]
    fn keep_alive_em_segundos_vai_como_numero() {
        let mut pedido = pedido();
        pedido.keep_alive = Some(KeepAlive::Segundos(-1));

        assert_eq!(pedido.para_json()["keep_alive"], -1);
    }

    #[test]
    fn modelo_sem_completion_nao_serve_para_conversa() {
        let so_embeddings = DetalhesModelo {
            capacidades: Some(vec!["embedding".into()]),
            janela_de_contexto: None,
            tem_metadados_de_raciocinio: false,
        };
        let desconhecido = DetalhesModelo {
            capacidades: None,
            janela_de_contexto: None,
            tem_metadados_de_raciocinio: false,
        };

        assert_eq!(
            so_embeddings.exige_conversa(),
            Err(ErroOllama::ModeloSemSuporte)
        );
        assert!(desconhecido.exige_conversa().is_ok());
    }

    #[test]
    fn detecta_suporte_a_raciocinio() {
        let com_capacidade = DetalhesModelo {
            capacidades: Some(vec!["completion".into(), "thinking".into()]),
            janela_de_contexto: None,
            tem_metadados_de_raciocinio: false,
        };
        let sem = DetalhesModelo {
            capacidades: Some(vec!["completion".into()]),
            janela_de_contexto: None,
            tem_metadados_de_raciocinio: false,
        };

        assert!(com_capacidade.suporta_raciocinio());
        assert!(!sem.suporta_raciocinio());
    }

    #[test]
    fn so_avisa_quando_a_versao_esta_abaixo_do_minimo() {
        let antiga = InfoVersao {
            versao: Versao::nova(0, 29, 0),
            atende_o_minimo: false,
        };
        let atual = InfoVersao {
            versao: VERSAO_MINIMA,
            atende_o_minimo: true,
        };

        assert!(matches!(
            antiga.aviso(),
            Some(ErroOllama::VersaoAntiga { .. })
        ));
        assert_eq!(atual.aviso(), None);
    }

    #[test]
    fn resposta_cortada_e_reconhecida() {
        let resposta = |motivo: &str| RespostaChat {
            texto: TextoParcial::default(),
            fim: MotivoFim::Concluida {
                motivo: Some(motivo.into()),
            },
        };

        assert!(resposta("length").foi_cortada());
        assert!(!resposta("stop").foi_cortada());
    }
}
