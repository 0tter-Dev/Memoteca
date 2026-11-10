//! Montagem do contexto enviado ao modelo a cada mensagem.
//!
//! Regras do contrato com o Ollama:
//!
//! * uma única mensagem `system`, **sempre a primeira**. Ela é montada **a cada envio**, e não
//!   só na primeira mensagem da conversa, e junta, nesta ordem: as instruções do projeto, as
//!   memórias globais e as memórias do projeto (o mais específico por último). Cada bloco que
//!   não tem conteúdo é omitido, e sem nenhum bloco não há mensagem `system`;
//! * depois, o histórico (`user` e `assistant`) só com `role` e `content`. O raciocínio
//!   (`thinking`) de respostas anteriores nunca é reenviado.
//!
//! As memórias são lidas do banco a cada envio, então editar ou excluir uma memória só muda as
//! gerações seguintes: as mensagens já geradas nunca são tocadas.

use crate::banco::{Memoria, Mensagem, Papel};
use crate::ollama::{MensagemPedido, PapelChat};

const TITULO_DAS_GLOBAIS: &str = "Memórias globais (valem para qualquer projeto):";
const TITULO_DAS_DO_PROJETO: &str = "Memórias deste projeto:";

/// Acima desta parte do `num_ctx` estimado, a tela avisa que o contexto está quase cheio.
pub const LIMITE_DO_AVISO: f64 = 0.8;

fn bloco_de_memorias(titulo: &str, memorias: &[Memoria]) -> Option<String> {
    if memorias.is_empty() {
        return None;
    }
    let mut bloco = String::from(titulo);
    for memoria in memorias {
        bloco.push_str("\n- ");
        // Uma memória de várias linhas continua com recuo, para continuar sendo um só item.
        bloco.push_str(&memoria.conteudo.trim().replace('\n', "\n  "));
    }
    Some(bloco)
}

/// O texto da mensagem `system`, ou `None` se não há instruções nem memórias.
pub fn texto_do_system(
    instrucoes_do_projeto: &str,
    memorias_globais: &[Memoria],
    memorias_do_projeto: &[Memoria],
) -> Option<String> {
    let mut blocos = Vec::new();
    if !instrucoes_do_projeto.trim().is_empty() {
        blocos.push(instrucoes_do_projeto.to_owned());
    }
    blocos.extend(bloco_de_memorias(TITULO_DAS_GLOBAIS, memorias_globais));
    blocos.extend(bloco_de_memorias(
        TITULO_DAS_DO_PROJETO,
        memorias_do_projeto,
    ));
    (!blocos.is_empty()).then(|| blocos.join("\n\n"))
}

pub fn montar_contexto(
    instrucoes_do_projeto: &str,
    memorias_globais: &[Memoria],
    memorias_do_projeto: &[Memoria],
    historico: &[Mensagem],
) -> Vec<MensagemPedido> {
    let mut mensagens = Vec::with_capacity(historico.len() + 1);

    if let Some(conteudo) =
        texto_do_system(instrucoes_do_projeto, memorias_globais, memorias_do_projeto)
    {
        mensagens.push(MensagemPedido {
            papel: PapelChat::Sistema,
            conteudo,
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

/// Estimativa do tamanho do contexto, em tokens. Não há tokenizador aqui, então conta 1 token
/// a cada 3 caracteres (mais que o comum em português, para errar para o lado seguro) e uns
/// poucos tokens por mensagem para a formatação do modelo. É só para avisar o usuário.
pub fn estimar_tokens(mensagens: &[MensagemPedido]) -> u64 {
    mensagens
        .iter()
        .map(|m| m.conteudo.chars().count().div_ceil(3) as u64 + 4)
        .sum()
}

/// O contexto montado está perto do `num_ctx` (ou já passou dele)?
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AvisoDeContexto {
    pub estimado: u64,
    pub limite: u64,
}

/// `Some` a partir de 80% do `num_ctx` estimado em uso.
pub fn avaliar_contexto(mensagens: &[MensagemPedido], num_ctx: u32) -> Option<AvisoDeContexto> {
    let limite = u64::from(num_ctx);
    let estimado = estimar_tokens(mensagens);
    (limite > 0 && estimado as f64 >= limite as f64 * LIMITE_DO_AVISO)
        .then_some(AvisoDeContexto { estimado, limite })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::{EscopoMemoria, EstadoMensagem};

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
            &[],
            &[],
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
            let contexto = montar_contexto(
                instrucoes,
                &[],
                &[],
                &[mensagem(Papel::Usuario, "Oi", None)],
            );

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

        let contexto = montar_contexto("Instruções.", &[], &[], &historico);

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

        let contexto = montar_contexto(instrucoes, &[], &[], &[]);

        assert_eq!(contexto[0].conteudo, instrucoes);
    }

    fn memoria(escopo: EscopoMemoria, conteudo: &str) -> Memoria {
        Memoria {
            id: "m".into(),
            projeto_id: (escopo == EscopoMemoria::Projeto).then(|| "p".to_owned()),
            escopo,
            conteudo: conteudo.into(),
            criado_em: String::new(),
            atualizado_em: String::new(),
        }
    }

    #[test]
    fn a_ordem_e_instrucoes_depois_globais_depois_do_projeto_numa_so_mensagem() {
        let globais = [memoria(EscopoMemoria::Global, "Mora em Recife")];
        let do_projeto = [
            memoria(EscopoMemoria::Projeto, "Usa Rust"),
            memoria(EscopoMemoria::Projeto, "Prefere exemplos curtos"),
        ];

        let contexto = montar_contexto(
            "Responda em português.",
            &globais,
            &do_projeto,
            &[mensagem(Papel::Usuario, "Oi", None)],
        );

        assert_eq!(contexto.len(), 2);
        assert_eq!(contexto[0].papel, PapelChat::Sistema);
        assert_eq!(
            contexto[0].conteudo,
            [
                "Responda em português.",
                "",
                "Memórias globais (valem para qualquer projeto):",
                "- Mora em Recife",
                "",
                "Memórias deste projeto:",
                "- Usa Rust",
                "- Prefere exemplos curtos",
            ]
            .join("\n")
        );
    }

    #[test]
    fn blocos_vazios_somem_e_sem_nada_nao_ha_mensagem_system() {
        let globais = [memoria(EscopoMemoria::Global, "Fato")];

        let so_memorias = montar_contexto("  ", &globais, &[], &[]);
        assert_eq!(so_memorias.len(), 1);
        assert_eq!(
            so_memorias[0].conteudo,
            "Memórias globais (valem para qualquer projeto):
- Fato"
        );

        assert!(montar_contexto("", &[], &[], &[]).is_empty());
    }

    #[test]
    fn memoria_de_varias_linhas_continua_sendo_um_so_item() {
        let globais = [memoria(
            EscopoMemoria::Global,
            "  linha 1
linha 2  ",
        )];

        let texto = texto_do_system("", &globais, &[]).unwrap();

        assert!(texto.ends_with(
            "- linha 1
  linha 2"
        ));
    }

    #[test]
    fn o_aviso_de_contexto_so_aparece_a_partir_de_80_por_cento() {
        let mensagem_de = |caracteres: usize| MensagemPedido {
            papel: PapelChat::Usuario,
            conteudo: "a".repeat(caracteres),
        };
        // 3 caracteres por token, mais 4 por mensagem: 2.396 caracteres = 799 + 4 = 803 tokens.
        assert_eq!(estimar_tokens(&[mensagem_de(2_396)]), 803);

        assert_eq!(avaliar_contexto(&[mensagem_de(300)], 1_000), None);
        let aviso = avaliar_contexto(&[mensagem_de(2_396)], 1_000).unwrap();
        assert_eq!((aviso.estimado, aviso.limite), (803, 1_000));
        // Passou do limite: também avisa.
        assert!(avaliar_contexto(&[mensagem_de(9_000)], 1_000).is_some());
    }
}
