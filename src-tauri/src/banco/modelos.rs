//! Entidades do banco. Datas são texto ISO 8601 em UTC, com milissegundos.

#[derive(Debug, Clone, PartialEq)]
pub struct Projeto {
    pub id: String,
    pub nome: String,
    pub system_prompt: String,
    pub modelo_padrao: Option<String>,
    pub criado_em: String,
    pub atualizado_em: String,
}

/// Campos editáveis de um projeto.
#[derive(Debug, Clone, PartialEq)]
pub struct DadosProjeto {
    pub nome: String,
    pub system_prompt: String,
    pub modelo_padrao: Option<String>,
}

/// O que será apagado junto com um projeto, para a tela de confirmação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumoExclusaoProjeto {
    pub conversas: u64,
    pub mensagens: u64,
    pub memorias: u64,
}

#[derive(Debug, Clone, PartialEq)]
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

/// Campos editáveis de uma conversa. Os valores padrão de modelo e parâmetros são
/// definidos na E10, então aqui todos podem ficar vazios.
#[derive(Debug, Clone, PartialEq)]
pub struct DadosConversa {
    pub titulo: String,
    pub modelo: Option<String>,
    pub temperature: Option<f64>,
    pub num_ctx: Option<i64>,
    pub keep_alive: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Usuario,
    Assistente,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Mensagem {
    pub id: String,
    pub conversa_id: String,
    /// Posição da mensagem na conversa, a partir de 1.
    pub posicao: i64,
    pub papel: Papel,
    pub conteudo: String,
    /// Raciocínio (`thinking`) do modelo, quando houver.
    pub raciocinio: Option<String>,
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
