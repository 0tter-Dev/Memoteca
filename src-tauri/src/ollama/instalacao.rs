//! Detecção de uma instalação existente do Ollama.
//!
//! O instalador oficial coloca a pasta do Ollama no `PATH` do usuário, mesmo quando a
//! instalação é feita em outro diretório. Por isso a busca olha primeiro o `PATH` e depois
//! o local padrão da instalação por usuário (`%LOCALAPPDATA%\Programs\Ollama`).

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const NOME_DO_EXECUTAVEL: &str = "ollama.exe";

/// Quem sabe dizer onde está o `ollama.exe`. Os testes usam versões falsas.
pub trait DetectorDeInstalacao: Send + Sync {
    /// Caminho do executável, se houver uma instalação.
    fn encontrar(&self) -> Option<PathBuf>;
}

/// Procura no ambiente real: `PATH` e `%LOCALAPPDATA%`.
pub struct DetectorReal;

impl DetectorDeInstalacao for DetectorReal {
    fn encontrar(&self) -> Option<PathBuf> {
        procurar_executavel(
            env::var_os("PATH"),
            env::var_os("LOCALAPPDATA").map(PathBuf::from),
        )
    }
}

/// Procura o `ollama.exe` nas pastas do `path` e, depois, no local padrão da instalação.
/// Devolve o primeiro que existir como arquivo.
pub fn procurar_executavel(
    path: Option<OsString>,
    local_app_data: Option<PathBuf>,
) -> Option<PathBuf> {
    let do_path = path
        .iter()
        .flat_map(env::split_paths)
        .filter(|pasta| !pasta.as_os_str().is_empty())
        .map(|pasta| pasta.join(NOME_DO_EXECUTAVEL));
    let padrao = local_app_data.iter().map(|base| {
        base.join("Programs")
            .join("Ollama")
            .join(NOME_DO_EXECUTAVEL)
    });

    do_path
        .chain(padrao)
        .find(|candidato| eh_arquivo(candidato))
}

fn eh_arquivo(caminho: &Path) -> bool {
    caminho.is_file()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    struct Pasta(PathBuf);

    impl Pasta {
        fn nova(nome: &str) -> Self {
            let caminho = env::temp_dir().join(format!(
                "memoteca-instalacao-{nome}-{}",
                crate::banco::id::novo_id()
            ));
            fs::create_dir_all(&caminho).unwrap();
            Self(caminho)
        }

        fn com_executavel(&self, subpasta: &str) -> PathBuf {
            let pasta = self.0.join(subpasta);
            fs::create_dir_all(&pasta).unwrap();
            fs::write(pasta.join(NOME_DO_EXECUTAVEL), b"").unwrap();
            pasta
        }
    }

    impl Drop for Pasta {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn path_de(pastas: &[PathBuf]) -> OsString {
        env::join_paths(pastas).unwrap()
    }

    #[test]
    fn acha_o_executavel_numa_pasta_do_path_mesmo_fora_do_local_padrao() {
        let base = Pasta::nova("path");
        let instalada = base.com_executavel("em/outro/disco/app");
        let sem_nada = base.0.join("vazia");
        fs::create_dir_all(&sem_nada).unwrap();

        let achado = procurar_executavel(Some(path_de(&[sem_nada, instalada.clone()])), None);

        assert_eq!(achado, Some(instalada.join(NOME_DO_EXECUTAVEL)));
    }

    #[test]
    fn acha_o_executavel_no_local_padrao_quando_nao_esta_no_path() {
        let base = Pasta::nova("padrao");
        let local = base.0.join("Local");
        let pasta = local.join("Programs").join("Ollama");
        fs::create_dir_all(&pasta).unwrap();
        fs::write(pasta.join(NOME_DO_EXECUTAVEL), b"").unwrap();

        let achado = procurar_executavel(Some(path_de(&[])), Some(local));

        assert_eq!(achado, Some(pasta.join(NOME_DO_EXECUTAVEL)));
    }

    #[test]
    fn o_path_tem_prioridade_sobre_o_local_padrao() {
        let base = Pasta::nova("prioridade");
        let no_path = base.com_executavel("no_path");
        let local = base.0.join("Local");
        let padrao = local.join("Programs").join("Ollama");
        fs::create_dir_all(&padrao).unwrap();
        fs::write(padrao.join(NOME_DO_EXECUTAVEL), b"").unwrap();

        let achado =
            procurar_executavel(Some(path_de(std::slice::from_ref(&no_path))), Some(local));

        assert_eq!(achado, Some(no_path.join(NOME_DO_EXECUTAVEL)));
    }

    #[test]
    fn sem_instalacao_devolve_nada() {
        let base = Pasta::nova("nada");
        let vazia = base.0.join("vazia");
        fs::create_dir_all(&vazia).unwrap();

        assert_eq!(
            procurar_executavel(Some(path_de(&[vazia])), Some(base.0.join("Local"))),
            None
        );
        assert_eq!(procurar_executavel(None, None), None);
    }

    #[test]
    fn uma_pasta_com_o_nome_do_executavel_nao_conta() {
        let base = Pasta::nova("pasta-falsa");
        let pasta = base.0.join("bin");
        fs::create_dir_all(pasta.join(NOME_DO_EXECUTAVEL)).unwrap();

        assert_eq!(procurar_executavel(Some(path_de(&[pasta])), None), None);
    }
}
