//! Montagem do contexto enviado ao modelo a cada mensagem.
//!
//! Regras do contrato com o Ollama:
//!
//! * uma única mensagem `system`, **sempre a primeira**, com as instruções do projeto. Ela é
//!   montada **a cada envio**, e não só na primeira mensagem da conversa;
//! * depois, o histórico (`user` e `assistant`) só com `role` e `content`. O raciocínio
//!   (`thinking`) de respostas anteriores nunca é reenviado.
//!
//! As memórias (projeto e globais) entram nesta mesma mensagem `system` a partir da E11.

use crate::banco::{Mensagem, Papel};
use crate::ollama::{MensagemPedido, PapelChat};

pub fn montar_contexto(instrucoes_do_projeto: &str, historico: &[Mensagem]) -> Vec<MensagemPedido> {
    let mut mensagens = Vec::with_capacity(historico.len() + 1);

    if !instrucoes_do_projeto.trim().is_empty() {
        mensagens.push(MensagemPedido {
            papel: PapelChat::Sistema,
            conteudo: instrucoes_do_projeto.to_owned(),
        });
    }
    for mensagem in historico {
        mensagens.push(MensagemPedido {
            papel: match mensagem.papel {
                Papel::Usuario => PapelChat::Usuario,
                Papel::Assistente => PapelChat::Assistente,
            },
            conteudo: mensagem.conteudo.clone(),
        });
    }
    mensagens
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::EstadoMensagem;

    fn mensagem(papel: Papel, conteudo: &str, raciocinio: Option<&str>) -> Mensagem {
        Mensagem {
            id: "i".into(),
            conversa_id: "c".into(),
            posicao: 1,
            papel,
            conteudo: conteudo.into(),
            raciocinio: raciocinio.map(str::to_owned),
            estado: EstadoMensagem::Completa,
            criado_em: String::new(),
            atualizado_em: String::new(),
        }
    }

    #[test]
    fn as_instrucoes_do_projeto_viram_a_primeira_mensagem_system() {
        let contexto = montar_contexto(
            "Responda em português.",
            &[mensagem(Papel::Usuario, "Oi", None)],
        );

        assert_eq!(contexto.len(), 2);
        assert_eq!(contexto[0].papel, PapelChat::Sistema);
        assert_eq!(contexto[0].conteudo, "Responda em português.");
        assert_eq!(contexto[1].papel, PapelChat::Usuario);
    }

    #[test]
    fn sem_instrucoes_nao_ha_mensagem_system() {
        for instrucoes in ["", "   ", "\n\t"] {
            let contexto = montar_contexto(instrucoes, &[mensagem(Papel::Usuario, "Oi", None)]);

            assert_eq!(contexto.len(), 1, "{instrucoes:?}");
            assert_eq!(contexto[0].papel, PapelChat::Usuario);
        }
    }

    #[test]
    fn o_historico_segue_na_ordem_e_o_raciocinio_nunca_e_reenviado() {
        let historico = [
            mensagem(Papel::Usuario, "Pergunta", None),
            mensagem(Papel::Assistente, "Resposta", Some("segredo do raciocínio")),
            mensagem(Papel::Usuario, "Outra", None),
        ];

        let contexto = montar_contexto("Instruções.", &historico);

        let papeis: Vec<PapelChat> = contexto.iter().map(|m| m.papel).collect();
        assert_eq!(
            papeis,
            [
                PapelChat::Sistema,
                PapelChat::Usuario,
                PapelChat::Assistente,
                PapelChat::Usuario
            ]
        );
        assert!(contexto.iter().all(|m| !m.conteudo.contains("segredo")));
    }

    #[test]
    fn as_instrucoes_vao_exatamente_como_estao_escritas() {
        let instrucoes = "  Linha 1\n\nLinha 3  ";

        let contexto = montar_contexto(instrucoes, &[]);

        assert_eq!(contexto[0].conteudo, instrucoes);
    }
}
