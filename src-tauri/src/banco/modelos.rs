//! Entidades do banco. Datas são texto ISO 8601 em UTC, com milissegundos.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Projeto {
    pub id: String,
    pub nome: String,
    pub system_prompt: String,
    pub modelo_padrao: Option<String>,
    pub criado_em: String,
    pub atualizado_em: String,
}

/// Campos editáveis de um projeto.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DadosProjeto {
    pub nome: String,
    pub system_prompt: String,
    pub modelo_padrao: Option<String>,
}

/// O que será apagado junto com um projeto, para a tela de confirmação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResumoExclusaoProjeto {
    pub conversas: u64,
    pub mensagens: u64,
    pub memorias: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversa {
    pub id: String,
    pub projeto_id: String,
    pub titulo: String,
    pub modelo: Option<String>,
    pub temperature: Option<f64>,
    pub num_ctx: Option<i64>,
    pub keep_alive: Option<String>,
    pub criado_em: String,
    pub atualizado_em: String,
}

/// Título de uma conversa recém-criada, até a primeira mensagem do usuário dar o título.
pub const TITULO_PADRAO_DA_CONVERSA: &str = "Nova conversa";
/// Parâmetros de uma conversa nova (decididos na E10). Cada conversa guarda os seus.
pub const TEMPERATURE_PADRAO: f64 = 0.7;
pub const NUM_CTX_PADRAO: i64 = 8192;
pub const KEEP_ALIVE_PADRAO: &str = "5m";

/// Campos editáveis de uma conversa. Modelo e parâmetros podem ficar vazios (sem valor).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DadosConversa {
    pub titulo: String,
    pub modelo: Option<String>,
    pub temperature: Option<f64>,
    pub num_ctx: Option<i64>,
    pub keep_alive: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Papel {
    #[serde(rename = "user")]
    Usuario,
    #[serde(rename = "assistant")]
    Assistente,
}

/// A mensagem está inteira ou foi guardada pela metade (resposta interrompida).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoMensagem {
    Completa,
    Interrompida,
}

impl EstadoMensagem {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Completa => "completa",
            Self::Interrompida => "interrompida",
        }
    }

    pub fn de_texto(texto: &str) -> Option<Self> {
        match texto {
            "completa" => Some(Self::Completa),
            "interrompida" => Some(Self::Interrompida),
            _ => None,
        }
    }
}

impl Papel {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Usuario => "user",
            Self::Assistente => "assistant",
        }
    }

    pub fn de_texto(texto: &str) -> Option<Self> {
        match texto {
            "user" => Some(Self::Usuario),
            "assistant" => Some(Self::Assistente),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mensagem {
    pub id: String,
    pub conversa_id: String,
    /// Posição da mensagem na conversa, a partir de 1.
    pub posicao: i64,
    pub papel: Papel,
    pub conteudo: String,
    /// Raciocínio (`thinking`) do modelo, quando houver.
    pub raciocinio: Option<String>,
    /// `Interrompida` quando a resposta foi guardada pela metade.
    pub estado: EstadoMensagem,
    pub criado_em: String,
    pub atualizado_em: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscopoMemoria {
    Global,
    Projeto,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Memoria {
    pub id: String,
    pub escopo: EscopoMemoria,
    /// Presente se, e somente se, o escopo é `Projeto`.
    pub projeto_id: Option<String>,
    pub conteudo: String,
    pub criado_em: String,
    pub atualizado_em: String,
}
