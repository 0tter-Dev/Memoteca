//! Cópia de segurança do banco antes de migrar.
//!
//! A cópia usa `VACUUM INTO`, que gera um arquivo consistente mesmo com o banco aberto.
//! Só os arquivos criados aqui (nome `copia-v<versão>-<milissegundos>.db`) são apagados na
//! limpeza. Nenhum outro arquivo da pasta é tocado.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::erro::ErroBanco;

/// Quantas cópias de segurança são mantidas na pasta.
pub const COPIAS_MANTIDAS: usize = 3;

const PREFIXO: &str = "copia-v";
const EXTENSAO: &str = ".db";

/// Cria a cópia de segurança em `pasta` e apaga as mais antigas além do limite.
pub fn criar(conexao: &Connection, pasta: &Path, versao_atual: u32) -> Result<PathBuf, ErroBanco> {
    fs::create_dir_all(pasta)?;

    let mut instante = milissegundos_atuais();
    let destino = loop {
        let candidato = pasta.join(format!("{PREFIXO}{versao_atual}-{instante:013}{EXTENSAO}"));
        if !candidato.exists() {
            break candidato;
        }
        instante += 1;
    };

    let caminho = destino
        .to_str()
        .ok_or_else(|| ErroBanco::Indisponivel("caminho da cópia não é texto válido".into()))?;
    conexao.execute("VACUUM INTO ?1", [caminho])?;

    limpar_antigas(pasta, COPIAS_MANTIDAS)?;
    Ok(destino)
}

/// Apaga as cópias mais antigas, mantendo as `manter` mais recentes.
pub fn limpar_antigas(pasta: &Path, manter: usize) -> Result<(), ErroBanco> {
    let mut copias: Vec<(u128, PathBuf)> = fs::read_dir(pasta)?
        .filter_map(Result::ok)
        .filter_map(|entrada| {
            let nome = entrada.file_name().into_string().ok()?;
            let instante = instante_do_nome(&nome)?;
            Some((instante, entrada.path()))
        })
        .collect();

    copias.sort();
    let excedentes = copias.len().saturating_sub(manter);
    for (_, caminho) in copias.into_iter().take(excedentes) {
        fs::remove_file(caminho)?;
    }
    Ok(())
}

/// Lê o instante de um nome `copia-v<versão>-<milissegundos>.db`. Outros nomes dão `None`.
fn instante_do_nome(nome: &str) -> Option<u128> {
    let sem_extensao = nome.strip_suffix(EXTENSAO)?;
    let resto = sem_extensao.strip_prefix(PREFIXO)?;
    let (versao, instante) = resto.split_once('-')?;
    versao.parse::<u32>().ok()?;
    instante.parse::<u128>().ok()
}

fn milissegundos_atuais() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duracao| duracao.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reconhece_apenas_os_nomes_das_copias() {
        assert_eq!(
            instante_do_nome("copia-v1-0001700000000.db"),
            Some(1_700_000_000)
        );
        assert_eq!(
            instante_do_nome("copia-v12-0001700000001.db"),
            Some(1_700_000_001)
        );
        assert_eq!(instante_do_nome("memoteca.db"), None);
        assert_eq!(instante_do_nome("copia-v1-0001700000000.txt"), None);
        assert_eq!(instante_do_nome("copia-vX-0001700000000.db"), None);
        assert_eq!(instante_do_nome("minhas-fotos.db"), None);
    }
}
