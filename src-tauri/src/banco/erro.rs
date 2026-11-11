//! Erros tipados do banco de dados.
//!
//! O texto técnico (`Display` e `Debug`) serve só para o log. O que o usuário vê é sempre
//! `mensagem_amigavel`, que nunca inclui texto do SQLite, SQL, caminhos ou códigos.

use std::fmt;

use rusqlite::ErrorCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroBanco {
    /// Não foi possível abrir ou usar o arquivo (travado, protegido, danificado).
    Indisponivel(String),
    /// Não há espaço em disco para gravar.
    SemEspaco,
    /// Uma migração falhou. O banco permaneceu como estava.
    MigracaoFalhou {
        versao: u32,
        detalhe: String,
        /// Se uma cópia de segurança foi feita antes de tentar migrar.
        copia_guardada: bool,
    },
    /// O banco foi criado por uma versão mais nova do app.
    VersaoMaisNova { encontrada: u32, suportada: u32 },
    /// O item pedido não existe.
    NaoEncontrado,
    /// A operação viola uma regra dos dados (por exemplo, memória de projeto sem projeto).
    Invalido(String),
    /// O usuário informou algo que não serve (nome vazio, texto grande demais). A mensagem já
    /// está em pt-BR e pode ser mostrada como está.
    Rejeitado(String),
    /// Falha inesperada.
    Interno(String),
}

impl ErroBanco {
    /// Mensagem em pt-BR para mostrar ao usuário.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::Indisponivel(_) => "Não foi possível abrir os dados do Memoteca. O arquivo \
                pode estar em uso por outro programa, protegido ou danificado. Seus dados não \
                foram apagados. Feche outros programas que possam estar usando o arquivo e \
                abra o Memoteca de novo."
                .to_owned(),
            Self::SemEspaco => "Não há espaço livre no disco para salvar seus dados. Libere \
                espaço e tente de novo."
                .to_owned(),
            Self::MigracaoFalhou { copia_guardada, .. } => {
                let mut texto = String::from(
                    "Não foi possível atualizar os dados do Memoteca para esta versão. Seus \
                     dados continuam como estavam.",
                );
                if *copia_guardada {
                    texto.push_str(" Uma cópia de segurança foi guardada antes da tentativa.");
                }
                texto.push_str(
                    " Tente abrir o Memoteca de novo. Se o problema continuar, atualize ou \
                     reinstale o aplicativo.",
                );
                texto
            }
            Self::VersaoMaisNova { .. } => "Estes dados foram criados por uma versão mais nova \
                do Memoteca. Atualize o aplicativo para abri-los. Nada foi alterado."
                .to_owned(),
            Self::NaoEncontrado => {
                "Não encontramos este item. Ele pode ter sido excluído. Escolha outro na lista.".to_owned()
            }
            Self::Invalido(_) => {
                "Não foi possível salvar: os dados informados não são válidos. Confira o que foi preenchido e tente de novo.".to_owned()
            }
            Self::Rejeitado(mensagem) => mensagem.clone(),
            Self::Interno(_) => {
                "Ocorreu um problema inesperado ao salvar seus dados. Tente de novo.".to_owned()
            }
        }
    }
}

impl fmt::Display for ErroBanco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Indisponivel(detalhe) => write!(f, "banco indisponível: {detalhe}"),
            Self::SemEspaco => write!(f, "sem espaço em disco"),
            Self::MigracaoFalhou {
                versao, detalhe, ..
            } => write!(f, "migração {versao} falhou: {detalhe}"),
            Self::VersaoMaisNova {
                encontrada,
                suportada,
            } => write!(
                f,
                "banco na versão {encontrada}, mas este app suporta até a {suportada}"
            ),
            Self::NaoEncontrado => write!(f, "item não encontrado"),
            Self::Invalido(detalhe) => write!(f, "dados inválidos: {detalhe}"),
            Self::Rejeitado(mensagem) => write!(f, "dados rejeitados: {mensagem}"),
            Self::Interno(detalhe) => write!(f, "erro interno: {detalhe}"),
        }
    }
}

impl std::error::Error for ErroBanco {}

impl From<rusqlite::Error> for ErroBanco {
    fn from(erro: rusqlite::Error) -> Self {
        let detalhe = erro.to_string();
        match erro {
            rusqlite::Error::QueryReturnedNoRows => Self::NaoEncontrado,
            rusqlite::Error::SqliteFailure(falha, _) => match falha.code {
                ErrorCode::ConstraintViolation => Self::Invalido(detalhe),
                ErrorCode::DiskFull => Self::SemEspaco,
                ErrorCode::DatabaseBusy
                | ErrorCode::DatabaseLocked
                | ErrorCode::NotADatabase
                | ErrorCode::DatabaseCorrupt
                | ErrorCode::CannotOpen
                | ErrorCode::ReadOnly
                | ErrorCode::PermissionDenied
                | ErrorCode::SystemIoFailure => Self::Indisponivel(detalhe),
                _ => Self::Interno(detalhe),
            },
            _ => Self::Interno(detalhe),
        }
    }
}

impl From<std::io::Error> for ErroBanco {
    fn from(erro: std::io::Error) -> Self {
        Self::Indisponivel(erro.to_string())
    }
}
