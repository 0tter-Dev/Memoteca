//! Erros tipados do cliente do Ollama.
//!
//! O texto técnico (`Display`) serve só para o log. O que o usuário vê é sempre
//! `mensagem_amigavel`. O texto cru de erro do Ollama nunca sobe para a interface.

use std::fmt;

use super::versao::Versao;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroOllama {
    /// Conexão recusada: nada está ouvindo na porta.
    OllamaForaDoAr,
    /// A porta responde, mas quem responde não é um Ollama.
    PortaOcupada,
    /// Ollama abaixo da versão mínima.
    VersaoAntiga { encontrada: Versao, minima: Versao },
    /// O modelo não está instalado.
    ModeloNaoEncontrado,
    /// O modelo não serve para conversa (por exemplo, só gera embeddings).
    ModeloSemSuporte,
    /// Nenhum byte chegou dentro do limite de ociosidade.
    TempoEsgotado,
    /// A conexão caiu antes de a resposta terminar.
    ConexaoInterrompida,
    /// A resposta não está no formato esperado (JSON inválido ou fora do esquema).
    RespostaInvalida(String),
    /// O Ollama devolveu um erro (HTTP 4xx/5xx ou linha `error` no stream).
    ErroDoOllama(String),
}

impl ErroOllama {
    /// Mensagem em pt-BR para mostrar ao usuário.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::OllamaForaDoAr => "O Ollama não está respondendo. Verifique se ele está em \
                execução e tente de novo."
                .to_owned(),
            Self::PortaOcupada => "Outro programa está usando a porta do Ollama (11434). Feche \
                esse programa e tente de novo."
                .to_owned(),
            Self::VersaoAntiga { encontrada, minima } => format!(
                "O Ollama instalado é a versão {encontrada}, e o Memoteca funciona melhor com a \
                 {minima} ou mais recente. Atualize o Ollama para usar todos os recursos."
            ),
            Self::ModeloNaoEncontrado => {
                "Este modelo não está instalado. Baixe o modelo e tente de novo.".to_owned()
            }
            Self::ModeloSemSuporte => {
                "Este modelo não serve para conversa. Escolha outro modelo.".to_owned()
            }
            Self::TempoEsgotado => {
                "O Ollama demorou demais para responder. Tente de novo.".to_owned()
            }
            Self::ConexaoInterrompida => {
                "A conexão com o Ollama caiu no meio da operação. Tente de novo.".to_owned()
            }
            Self::RespostaInvalida(_) => {
                "O Ollama enviou uma resposta inesperada. Tente de novo.".to_owned()
            }
            Self::ErroDoOllama(_) => {
                "Não foi possível concluir a operação no Ollama. Tente de novo.".to_owned()
            }
        }
    }
}

impl fmt::Display for ErroOllama {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OllamaForaDoAr => write!(f, "conexão recusada"),
            Self::PortaOcupada => write!(f, "a porta responde, mas não é um Ollama"),
            Self::VersaoAntiga { encontrada, minima } => {
                write!(f, "versão {encontrada} abaixo da mínima {minima}")
            }
            Self::ModeloNaoEncontrado => write!(f, "modelo não encontrado"),
            Self::ModeloSemSuporte => write!(f, "modelo sem suporte a conversa"),
            Self::TempoEsgotado => write!(f, "tempo esgotado sem receber dados"),
            Self::ConexaoInterrompida => write!(f, "conexão interrompida"),
            Self::RespostaInvalida(detalhe) => write!(f, "resposta inválida: {detalhe}"),
            Self::ErroDoOllama(detalhe) => write!(f, "erro do Ollama: {detalhe}"),
        }
    }
}

impl std::error::Error for ErroOllama {}

/// Texto que já chegou de uma resposta de chat. Preservado mesmo quando a resposta falha
/// no meio, para que o usuário não perca o que já foi gerado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextoParcial {
    pub conteudo: String,
    /// Raciocínio (`thinking`), só se o modelo emitiu algum.
    pub raciocinio: Option<String>,
}

/// Falha de uma conversa, com o que já tinha chegado antes dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalhaChat {
    pub erro: ErroOllama,
    pub parcial: TextoParcial,
}

impl fmt::Display for FalhaChat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.erro)
    }
}

impl std::error::Error for FalhaChat {}
