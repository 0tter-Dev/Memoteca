//! Conversas e chat com streaming (E10): título automático, montagem do contexto, geração da
//! resposta (responder e regenerar) e o controle de "uma geração por vez".

pub mod contexto;
pub mod controle;
pub mod geracao;
pub mod titulo;

use std::sync::Mutex;

use crate::banco::modelos::TITULO_PADRAO_DA_CONVERSA;
use crate::banco::{Banco, DadosConversa, ErroBanco, Mensagem, Papel};

pub use contexto::AvisoDeContexto;
pub use controle::{ControleDeGeracao, VagaDeGeracao};
pub use geracao::{DesfechoDaGeracao, ErroGeracao, FalhaDeGeracao, ModoDeGeracao, gerar};
pub use titulo::titulo_da_mensagem;

/// Tamanho máximo de uma mensagem do usuário, em caracteres.
pub const MENSAGEM_MAXIMA: usize = 100_000;

/// Grava a mensagem do usuário **antes** de qualquer chamada ao Ollama, então ela nunca se
/// perde. Se for a primeira mensagem e a conversa ainda tiver o título padrão, o título passa
/// a ser o início da mensagem. Um título que o usuário já editou nunca é sobrescrito.
pub fn registrar_mensagem_do_usuario(
    banco: &Mutex<Banco>,
    conversa_id: &str,
    texto: &str,
) -> Result<Mensagem, ErroBanco> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Err(ErroBanco::Rejeitado(
            "Escreva uma mensagem antes de enviar.".to_owned(),
        ));
    }
    if texto.chars().count() > MENSAGEM_MAXIMA {
        return Err(ErroBanco::Rejeitado(
            "A mensagem é grande demais. Divida o texto em partes menores.".to_owned(),
        ));
    }

    let banco = banco
        .lock()
        .map_err(|_| ErroBanco::Interno("banco em uso por outra operação".into()))?;
    let conversa = banco.obter_conversa(conversa_id)?;
    let primeira = banco.listar_mensagens(conversa_id)?.is_empty();

    let mensagem = banco.adicionar_mensagem(conversa_id, Papel::Usuario, texto, None)?;
    if primeira && conversa.titulo == TITULO_PADRAO_DA_CONVERSA {
        banco.atualizar_conversa(
            conversa_id,
            &DadosConversa {
                titulo: titulo_da_mensagem(texto),
                modelo: conversa.modelo,
                temperature: conversa.temperature,
                num_ctx: conversa.num_ctx,
                keep_alive: conversa.keep_alive,
            },
        )?;
    }
    banco.tocar_conversa(conversa_id)?;
    Ok(mensagem)
}
