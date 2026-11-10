//! Persistência local em SQLite (E04).
//!
//! `Banco` abre o arquivo, aplica as migrações pendentes e expõe as operações de dados
//! (projetos, conversas, mensagens, memórias e configurações). Todas as falhas viram
//! `ErroBanco`, cuja `mensagem_amigavel` é o único texto que pode chegar ao usuário.

pub mod backup;
pub mod erro;
pub mod id;
pub mod migracoes;
pub mod modelos;
mod repositorio;

use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

pub use erro::ErroBanco;
pub use modelos::{
    Conversa, DadosConversa, DadosProjeto, EscopoMemoria, EstadoMensagem, Memoria, Mensagem, Papel,
    Projeto, ResumoExclusaoProjeto,
};

use migracoes::{MIGRACOES, Migracao};

/// Quanto tempo esperar por um banco travado por outro processo antes de desistir.
const ESPERA_POR_BANCO_TRAVADO: Duration = Duration::from_secs(5);

pub struct Banco {
    conexao: Connection,
}

impl Banco {
    /// Abre (ou cria) o banco em `arquivo` e aplica as migrações do app. Se for preciso
    /// migrar um banco existente, a cópia de segurança vai para `pasta_backups`.
    pub fn abrir(arquivo: &Path, pasta_backups: &Path) -> Result<Self, ErroBanco> {
        Self::abrir_com(arquivo, MIGRACOES, Some(pasta_backups))
    }

    /// Como `abrir`, mas com uma lista de migrações própria. Existe para os testes de
    /// migração (versão anterior, falha no meio).
    pub fn abrir_com(
        arquivo: &Path,
        migracoes: &[Migracao],
        pasta_backups: Option<&Path>,
    ) -> Result<Self, ErroBanco> {
        if let Some(pasta) = arquivo.parent() {
            fs::create_dir_all(pasta)?;
        }
        let conexao = Connection::open(arquivo)?;
        Self::preparar(conexao, migracoes, pasta_backups)
    }

    /// Banco temporário, só em memória. Para testes.
    pub fn em_memoria() -> Result<Self, ErroBanco> {
        Self::preparar(Connection::open_in_memory()?, MIGRACOES, None)
    }

    fn preparar(
        mut conexao: Connection,
        migracoes: &[Migracao],
        pasta_backups: Option<&Path>,
    ) -> Result<Self, ErroBanco> {
        conexao.busy_timeout(ESPERA_POR_BANCO_TRAVADO)?;
        conexao.pragma_update(None, "foreign_keys", true)?;
        migracoes::aplicar(&mut conexao, migracoes, pasta_backups)?;
        Ok(Self { conexao })
    }

    /// Versão do esquema gravada no banco.
    pub fn versao_do_esquema(&self) -> Result<u32, ErroBanco> {
        migracoes::versao_do_banco(&self.conexao)
    }
}
