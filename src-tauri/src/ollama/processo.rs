//! Início e encerramento do processo `ollama serve`.
//!
//! `IniciadorDeProcesso` e `ProcessoFilho` existem para que os testes troquem o processo
//! real por um falso. A versão real coloca cada processo iniciado num Job Object do Windows
//! (ver `job.rs`), de modo que ele morra junto com o Memoteca, inclusive se o Memoteca for
//! finalizado à força.

use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use super::job::JobDeEncerramento;

/// Um processo que o Memoteca iniciou.
pub trait ProcessoFilho: Send {
    /// `Some(código)` se o processo já terminou, `None` se continua rodando.
    fn saiu(&mut self) -> io::Result<Option<i32>>;

    /// Encerra o processo (e seus filhos) e espera ele terminar. Não faz nada se já saiu.
    fn encerrar(&mut self) -> io::Result<()>;
}

pub trait IniciadorDeProcesso: Send + Sync {
    fn iniciar(&self, executavel: &Path, argumentos: &[&str])
    -> io::Result<Box<dyn ProcessoFilho>>;
}

/// Inicia processos de verdade.
pub struct IniciadorReal;

impl IniciadorDeProcesso for IniciadorReal {
    fn iniciar(
        &self,
        executavel: &Path,
        argumentos: &[&str],
    ) -> io::Result<Box<dyn ProcessoFilho>> {
        // O job é criado antes do processo. Se não for possível protegê-lo, nada é iniciado:
        // é melhor falhar do que deixar um processo órfão.
        #[cfg(windows)]
        let job = JobDeEncerramento::novo()?;

        let mut comando = Command::new(executavel);
        comando
            .args(argumentos)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Sem janela de console piscando na tela.
            const CRIAR_SEM_JANELA: u32 = 0x0800_0000;
            comando.creation_flags(CRIAR_SEM_JANELA);
        }

        let mut filho = comando.spawn()?;

        #[cfg(windows)]
        if let Err(erro) = job.adicionar(&filho) {
            let _ = filho.kill();
            let _ = filho.wait();
            return Err(erro);
        }

        Ok(Box::new(FilhoReal {
            filho,
            #[cfg(windows)]
            job: Some(job),
        }))
    }
}

struct FilhoReal {
    filho: Child,
    #[cfg(windows)]
    job: Option<JobDeEncerramento>,
}

impl ProcessoFilho for FilhoReal {
    fn saiu(&mut self) -> io::Result<Option<i32>> {
        Ok(self
            .filho
            .try_wait()?
            .map(|estado| estado.code().unwrap_or(-1)))
    }

    fn encerrar(&mut self) -> io::Result<()> {
        // Fechar o job encerra o processo e todos os descendentes dele (por exemplo, o
        // executor do modelo que o Ollama inicia).
        #[cfg(windows)]
        drop(self.job.take());

        if self.filho.try_wait()?.is_none() {
            // Já pode ter sido encerrado pelo job. O erro de `kill` aqui não importa.
            let _ = self.filho.kill();
        }
        self.filho.wait()?;
        Ok(())
    }
}
