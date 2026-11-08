//! Espaço livre em disco, antes de baixar e instalar.

use std::io;
use std::path::{Component, Path};

/// Quanto espaço livre existe num caminho. Os testes usam versões falsas.
pub trait EspacoEmDisco: Send + Sync {
    /// Bytes livres para o usuário na unidade que contém `pasta`.
    fn livre_em(&self, pasta: &Path) -> io::Result<u64>;
}

/// Pergunta ao Windows.
pub struct EspacoReal;

#[cfg(windows)]
impl EspacoEmDisco for EspacoReal {
    fn livre_em(&self, pasta: &Path) -> io::Result<u64> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        // A pasta pode ainda não existir: usa a primeira que existir acima dela.
        let existente = pasta
            .ancestors()
            .find(|candidato| candidato.exists())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "nenhuma pasta existente"))?;
        let caminho: Vec<u16> = existente
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut livre_para_o_usuario: u64 = 0;
        // SAFETY: `caminho` termina em zero e vive durante a chamada. O primeiro ponteiro de
        // saída é válido, e os outros dois são nulos, o que a API permite.
        let resultado = unsafe {
            GetDiskFreeSpaceExW(
                caminho.as_ptr(),
                &mut livre_para_o_usuario,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if resultado == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(livre_para_o_usuario)
    }
}

#[cfg(not(windows))]
impl EspacoEmDisco for EspacoReal {
    fn livre_em(&self, _pasta: &Path) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "só há suporte no Windows",
        ))
    }
}

/// As duas pastas ficam na mesma unidade (`C:`, por exemplo)? Compara o prefixo do caminho
/// sem diferenciar maiúsculas de minúsculas. Sem prefixo (caminho relativo), considera que
/// sim, que é o lado seguro para somar as necessidades.
pub fn mesma_unidade(a: &Path, b: &Path) -> bool {
    match (prefixo(a), prefixo(b)) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(&y),
        _ => true,
    }
}

fn prefixo(caminho: &Path) -> Option<String> {
    caminho
        .components()
        .find_map(|componente| match componente {
            Component::Prefix(prefixo) => Some(prefixo.as_os_str().to_string_lossy().into_owned()),
            _ => None,
        })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn mesma_unidade_ignora_maiusculas_e_distingue_unidades() {
        assert!(mesma_unidade(Path::new(r"C:\a\b"), Path::new(r"c:\x\y")));
        assert!(!mesma_unidade(Path::new(r"C:\a"), Path::new(r"E:\a")));
    }

    #[cfg(windows)]
    #[test]
    fn o_espaco_real_funciona_para_uma_pasta_que_ainda_nao_existe() {
        let pasta = std::env::temp_dir()
            .join("memoteca-pasta-que-nao-existe")
            .join("filha");

        let livre = EspacoReal.livre_em(&pasta).expect("ler o espaço livre");

        assert!(livre > 0);
    }
}
