//! Só uma geração de resposta roda por vez (uma conversa ativa por vez). Este controle
//! guarda o cancelamento da geração em andamento.

use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

use super::geracao::ErroGeracao;

#[derive(Default)]
pub struct ControleDeGeracao {
    em_andamento: Mutex<Option<CancellationToken>>,
}

/// A vaga da geração em andamento. Ao sair de escopo, libera a vaga, em qualquer desfecho.
pub struct VagaDeGeracao<'a> {
    controle: &'a ControleDeGeracao,
    cancelamento: CancellationToken,
}

impl VagaDeGeracao<'_> {
    pub fn cancelamento(&self) -> &CancellationToken {
        &self.cancelamento
    }
}

impl Drop for VagaDeGeracao<'_> {
    fn drop(&mut self) {
        if let Ok(mut vaga) = self.controle.em_andamento.lock() {
            *vaga = None;
        }
    }
}

impl ControleDeGeracao {
    /// Reserva a vaga, ou `Err(JaGerando)` se já houver uma geração em andamento.
    pub fn iniciar(&self) -> Result<VagaDeGeracao<'_>, ErroGeracao> {
        let mut vaga = self
            .em_andamento
            .lock()
            .map_err(|_| ErroGeracao::JaGerando)?;
        if vaga.is_some() {
            return Err(ErroGeracao::JaGerando);
        }
        let cancelamento = CancellationToken::new();
        *vaga = Some(cancelamento.clone());
        Ok(VagaDeGeracao {
            controle: self,
            cancelamento,
        })
    }

    /// Cancela a geração em andamento. Não faz nada se não houver nenhuma.
    pub fn cancelar(&self) {
        if let Ok(vaga) = self.em_andamento.lock()
            && let Some(cancelamento) = vaga.as_ref()
        {
            cancelamento.cancel();
        }
    }

    pub fn gerando(&self) -> bool {
        self.em_andamento.lock().is_ok_and(|vaga| vaga.is_some())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_uma_geracao_por_vez_e_a_vaga_e_liberada_ao_terminar() {
        let controle = ControleDeGeracao::default();

        let primeira = controle.iniciar().expect("a primeira reserva a vaga");
        assert!(controle.gerando());
        assert!(matches!(controle.iniciar(), Err(ErroGeracao::JaGerando)));

        drop(primeira);

        assert!(!controle.gerando());
        assert!(controle.iniciar().is_ok());
    }

    #[test]
    fn cancelar_aciona_o_cancelamento_da_geracao_em_andamento() {
        let controle = ControleDeGeracao::default();
        let vaga = controle.iniciar().unwrap();
        assert!(!vaga.cancelamento().is_cancelled());

        controle.cancelar();

        assert!(vaga.cancelamento().is_cancelled());
    }

    #[test]
    fn cancelar_sem_geracao_nao_faz_nada() {
        let controle = ControleDeGeracao::default();

        controle.cancelar();

        assert!(!controle.gerando());
    }
}
