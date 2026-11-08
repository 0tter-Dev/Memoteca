//! Execução do instalador do Ollama e do desinstalador usado no rollback.
//!
//! `ExecutorDoInstalador` existe para que os testes troquem a instalação real por uma falsa.
//! A versão real roda o instalador do Inno Setup do Ollama no modo totalmente silencioso, na
//! pasta padrão do usuário e **sem pedir elevação**: o Memoteca nunca eleva por conta própria.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

/// Argumentos do instalador (Inno Setup): sem janelas, sem caixas de mensagem, sem reiniciar
/// o computador e sem a página inicial.
pub const ARGUMENTOS_DO_INSTALADOR: [&str; 4] =
    ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"];

/// Argumentos do desinstalador (Inno Setup), na mesma linha do instalador.
pub const ARGUMENTOS_DO_DESINSTALADOR: [&str; 3] =
    ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];

/// Código de erro do Windows para "a operação exige elevação".
const ERRO_ELEVACAO_NECESSARIA: i32 = 740;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDeExecucao {
    /// O Windows exigiu permissão de administrador.
    ElevacaoNecessaria,
    /// O instalador terminou com este código de saída.
    SaiuComCodigo(i32),
    /// Não foi possível iniciar o processo.
    NaoIniciou(String),
    /// Foi cancelado enquanto rodava. O processo foi encerrado.
    Cancelado,
    /// Passou do prazo máximo. O processo foi encerrado.
    PrazoEsgotado,
}

pub trait ExecutorDoInstalador: Send + Sync {
    /// Executa o instalador e espera ele terminar. Bloqueia. Encerra o processo se
    /// `cancelamento` for acionado.
    fn instalar(
        &self,
        instalador: &Path,
        cancelamento: &CancellationToken,
    ) -> Result<(), ErroDeExecucao>;

    /// Executa o desinstalador (`unins000.exe`) para desfazer uma instalação parcial.
    /// Bloqueia até terminar.
    fn desinstalar(&self, desinstalador: &Path) -> io::Result<()>;
}

/// Executa processos de verdade.
pub struct ExecutorReal {
    /// Prazo máximo da instalação. O download de 1,7 GB não conta aqui.
    pub prazo_da_instalacao: Duration,
}

impl Default for ExecutorReal {
    fn default() -> Self {
        Self {
            prazo_da_instalacao: Duration::from_secs(20 * 60),
        }
    }
}

fn comando(executavel: &Path, argumentos: &[&str]) -> Command {
    let mut comando = Command::new(executavel);
    comando
        .args(argumentos)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CRIAR_SEM_JANELA: u32 = 0x0800_0000;
        comando.creation_flags(CRIAR_SEM_JANELA);
    }
    comando
}

impl ExecutorDoInstalador for ExecutorReal {
    fn instalar(
        &self,
        instalador: &Path,
        cancelamento: &CancellationToken,
    ) -> Result<(), ErroDeExecucao> {
        let mut filho = comando(instalador, &ARGUMENTOS_DO_INSTALADOR)
            .spawn()
            .map_err(|erro| {
                if erro.raw_os_error() == Some(ERRO_ELEVACAO_NECESSARIA) {
                    ErroDeExecucao::ElevacaoNecessaria
                } else {
                    ErroDeExecucao::NaoIniciou(erro.to_string())
                }
            })?;

        let limite = Instant::now() + self.prazo_da_instalacao;
        loop {
            match filho.try_wait() {
                Ok(Some(estado)) => {
                    return match estado.code() {
                        Some(0) => Ok(()),
                        Some(codigo) => Err(ErroDeExecucao::SaiuComCodigo(codigo)),
                        None => Err(ErroDeExecucao::SaiuComCodigo(-1)),
                    };
                }
                Ok(None) => {}
                Err(erro) => return Err(ErroDeExecucao::NaoIniciou(erro.to_string())),
            }

            let interrupcao = if cancelamento.is_cancelled() {
                Some(ErroDeExecucao::Cancelado)
            } else if Instant::now() >= limite {
                Some(ErroDeExecucao::PrazoEsgotado)
            } else {
                None
            };
            if let Some(interrupcao) = interrupcao {
                let _ = filho.kill();
                let _ = filho.wait();
                return Err(interrupcao);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn desinstalar(&self, desinstalador: &Path) -> io::Result<()> {
        let estado = comando(desinstalador, &ARGUMENTOS_DO_DESINSTALADOR).status()?;
        if estado.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "o desinstalador terminou com {estado}"
            )))
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_argumentos_sao_os_do_modo_totalmente_silencioso_sem_elevar() {
        assert_eq!(
            ARGUMENTOS_DO_INSTALADOR,
            ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"]
        );
        // Nenhum argumento muda a pasta de instalação nem pede administrador.
        for argumento in ARGUMENTOS_DO_INSTALADOR {
            assert!(!argumento.starts_with("/DIR"));
            assert!(!argumento.to_lowercase().contains("admin"));
        }
    }

    #[test]
    fn um_executavel_que_nao_existe_nao_inicia() {
        let executor = ExecutorReal::default();

        let resultado = executor.instalar(
            Path::new(r"C:\memoteca\nao\existe\OllamaSetup.exe"),
            &CancellationToken::new(),
        );

        assert!(matches!(resultado, Err(ErroDeExecucao::NaoIniciou(_))));
    }
}
