//! Job Object do Windows que encerra os processos do Ollama junto com o Memoteca.
//!
//! O job é criado com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: quando o último handle para ele
//! é fechado, o Windows encerra todos os processos que estão no job. O handle fica aberto
//! enquanto o Memoteca existe e é fechado pelo próprio sistema se o processo for finalizado
//! à força (Gerenciador de Tarefas, queda). Os processos filhos do Ollama, como o executor
//! do modelo, herdam o job e também são encerrados.
//!
//! Este é o único módulo com código `unsafe`.

use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::ptr::null;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

pub struct JobDeEncerramento {
    handle: HANDLE,
}

// O handle de um job é só um número de objeto do kernel, e as chamadas usadas são seguras
// para uso em várias threads.
unsafe impl Send for JobDeEncerramento {}
unsafe impl Sync for JobDeEncerramento {}

impl JobDeEncerramento {
    /// Cria um job que mata seus processos quando for fechado.
    pub fn novo() -> io::Result<Self> {
        // SAFETY: os argumentos são ponteiros nulos válidos (sem atributos de segurança e
        // sem nome), e o resultado é verificado antes de ser usado.
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self { handle };

        // SAFETY: a estrutura é zerada, que é um estado válido para ela, e só então o
        // limite é ligado. O ponteiro e o tamanho passados correspondem à estrutura.
        let mut informacao: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        informacao.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configurado = unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&raw const informacao).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configurado == 0 {
            // `job` é descartado aqui e fecha o handle.
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Coloca o processo (e, a partir daí, os filhos dele) dentro do job.
    pub fn adicionar(&self, processo: &Child) -> io::Result<()> {
        // SAFETY: os dois handles são válidos: o do job pertence a `self`, e o do processo
        // pertence a `processo`, que está vivo durante a chamada.
        let adicionado =
            unsafe { AssignProcessToJobObject(self.handle, processo.as_raw_handle().cast()) };
        if adicionado == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for JobDeEncerramento {
    fn drop(&mut self) {
        // SAFETY: o handle foi criado por `CreateJobObjectW` e só é fechado aqui. Fechar o
        // último handle encerra os processos que ainda estiverem no job.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
