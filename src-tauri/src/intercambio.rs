//! Exportar e importar os dados (E13): a parte que mexe em arquivos.
//!
//! O conteúdo do backup, a verificação e a mesclagem no banco ficam em `banco::intercambio`.
//! Aqui ficam a gravação do arquivo, **sem nunca deixar um arquivo pela metade**, e a leitura
//! do arquivo escolhido pelo usuário.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::banco::intercambio::{Backup, ErroDeBackup};
use crate::banco::{Banco, ErroBanco};

/// Acima disto o arquivo não pode ser um backup. Evita ler um arquivo enorme na memória.
pub const TAMANHO_MAXIMO_DO_ARQUIVO: u64 = 200 * 1024 * 1024;

const SUFIXO_TEMPORARIO: &str = ".memoteca-tmp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDeExportacao {
    /// O Memoteca não tem permissão para gravar nesse local.
    SemPermissao,
    /// Não há espaço livre no disco.
    SemEspaco,
    /// A pasta não existe ou o caminho não serve.
    DestinoInvalido,
    Banco(ErroBanco),
    Outro(String),
}

impl ErroDeExportacao {
    /// Mensagem em pt-BR para o usuário, sem caminhos nem texto do sistema.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::SemPermissao => "O Memoteca não tem permissão para gravar nesse local. \
                Escolha outra pasta, como Documentos. Nenhum arquivo foi criado."
                .to_owned(),
            Self::SemEspaco => "Não há espaço livre no disco para gravar o backup. Libere \
                espaço ou escolha outro disco. Nenhum arquivo foi criado."
                .to_owned(),
            Self::DestinoInvalido => "Não foi possível gravar nesse local. Escolha outra pasta. \
                Nenhum arquivo foi criado."
                .to_owned(),
            Self::Banco(erro) => erro.mensagem_amigavel(),
            Self::Outro(_) => "Não foi possível gravar o backup. Tente de novo ou escolha \
                outro local. Nenhum arquivo foi criado."
                .to_owned(),
        }
    }
}

impl std::fmt::Display for ErroDeExportacao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemPermissao => write!(f, "sem permissão para gravar"),
            Self::SemEspaco => write!(f, "sem espaço em disco"),
            Self::DestinoInvalido => write!(f, "destino inválido"),
            Self::Banco(erro) => write!(f, "{erro}"),
            Self::Outro(detalhe) => write!(f, "falha ao gravar: {detalhe}"),
        }
    }
}

impl std::error::Error for ErroDeExportacao {}

impl From<ErroBanco> for ErroDeExportacao {
    fn from(erro: ErroBanco) -> Self {
        Self::Banco(erro)
    }
}

impl From<io::Error> for ErroDeExportacao {
    fn from(erro: io::Error) -> Self {
        // 112 e 39 são "disco cheio" no Windows.
        if erro.kind() == io::ErrorKind::StorageFull
            || matches!(erro.raw_os_error(), Some(112 | 39))
        {
            return Self::SemEspaco;
        }
        match erro.kind() {
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                Self::SemPermissao
            }
            io::ErrorKind::NotFound
            | io::ErrorKind::InvalidInput
            | io::ErrorKind::NotADirectory => Self::DestinoInvalido,
            _ => Self::Outro(erro.to_string()),
        }
    }
}

/// Quantos itens foram para o arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoDaExportacao {
    pub projetos: u64,
    pub conversas: u64,
    pub mensagens: u64,
    pub memorias: u64,
}

fn arquivo_temporario(destino: &Path) -> PathBuf {
    let mut nome = destino.as_os_str().to_owned();
    nome.push(SUFIXO_TEMPORARIO);
    PathBuf::from(nome)
}

/// Grava o arquivo de forma atômica: escreve num arquivo temporário ao lado, confere que
/// chegou ao disco e só então troca pelo destino. Se qualquer passo falhar, o temporário é
/// apagado e **o destino fica como estava** (um backup antigo no mesmo lugar não se perde).
pub fn gravar_atomico_com(
    destino: &Path,
    escrever: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), ErroDeExportacao> {
    let temporario = arquivo_temporario(destino);
    let resultado = (|| -> io::Result<()> {
        let mut arquivo = File::create(&temporario)?;
        escrever(&mut arquivo)?;
        arquivo.flush()?;
        arquivo.sync_all()?;
        drop(arquivo);
        fs::rename(&temporario, destino)
    })();
    if let Err(erro) = resultado {
        let _ = fs::remove_file(&temporario);
        return Err(erro.into());
    }
    Ok(())
}

/// Grava `bytes` em `destino`, de forma atômica.
pub fn gravar_atomico(destino: &Path, bytes: &[u8]) -> Result<(), ErroDeExportacao> {
    gravar_atomico_com(destino, |arquivo| arquivo.write_all(bytes))
}

/// Grava um backup já montado em `destino` (um arquivo JSON com a versão do formato).
pub fn gravar_backup(
    backup: &Backup,
    destino: &Path,
) -> Result<ResumoDaExportacao, ErroDeExportacao> {
    let bytes = serde_json::to_vec_pretty(backup)
        .map_err(|erro| ErroDeExportacao::Outro(erro.to_string()))?;
    gravar_atomico(destino, &bytes)?;
    Ok(ResumoDaExportacao {
        projetos: backup.projetos.len() as u64,
        conversas: backup.conversas.len() as u64,
        mensagens: backup.mensagens.len() as u64,
        memorias: backup.memorias.len() as u64,
    })
}

/// Monta o backup e o grava em `destino`.
pub fn exportar_para(
    banco: &Banco,
    destino: &Path,
) -> Result<ResumoDaExportacao, ErroDeExportacao> {
    gravar_backup(&banco.montar_backup()?, destino)
}

/// Lê e **verifica** um arquivo de backup. Nada é gravado no banco.
pub fn ler_backup(caminho: &Path) -> Result<Backup, ErroDeBackup> {
    ler_backup_limitado(caminho, TAMANHO_MAXIMO_DO_ARQUIVO)
}

/// Como `ler_backup`, com o limite de tamanho do arquivo escolhido por quem chama.
pub fn ler_backup_limitado(caminho: &Path, limite: u64) -> Result<Backup, ErroDeBackup> {
    let arquivo =
        File::open(caminho).map_err(|erro| ErroDeBackup::ArquivoIlegivel(erro.to_string()))?;
    let tamanho = arquivo
        .metadata()
        .map_err(|erro| ErroDeBackup::ArquivoIlegivel(erro.to_string()))?
        .len();
    if tamanho > limite {
        return Err(ErroDeBackup::ArquivoGrandeDemais);
    }
    let bytes =
        fs::read(caminho).map_err(|erro| ErroDeBackup::ArquivoIlegivel(erro.to_string()))?;
    Backup::ler(&bytes)
}
