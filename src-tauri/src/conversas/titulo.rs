//! Título automático de uma conversa, tirado da primeira mensagem do usuário.
//!
//! Regra (decidida na E10): primeira linha com texto, espaços repetidos viram um só, no
//! máximo 40 caracteres, cortando no fim de uma palavra e acrescentando "…" quando cortou.

use crate::banco::modelos::TITULO_PADRAO_DA_CONVERSA;

/// Tamanho máximo do título automático, em caracteres (sem contar a reticência).
pub const TITULO_AUTOMATICO_MAXIMO: usize = 40;

pub fn titulo_da_mensagem(texto: &str) -> String {
    let Some(linha) = texto.lines().find(|linha| !linha.trim().is_empty()) else {
        return TITULO_PADRAO_DA_CONVERSA.to_owned();
    };
    let limpa = linha.split_whitespace().collect::<Vec<_>>().join(" ");
    let caracteres: Vec<char> = limpa.chars().collect();
    if caracteres.len() <= TITULO_AUTOMATICO_MAXIMO {
        return limpa;
    }

    // Corta no fim de uma palavra: se o caractere seguinte ao limite já é um espaço, o corte é
    // limpo; senão, recua até o último espaço dentro do limite (se não houver, corta no limite).
    let corte = if caracteres[TITULO_AUTOMATICO_MAXIMO].is_whitespace() {
        TITULO_AUTOMATICO_MAXIMO
    } else {
        caracteres[..TITULO_AUTOMATICO_MAXIMO]
            .iter()
            .rposition(|c| c.is_whitespace())
            .filter(|posicao| *posicao > 0)
            .unwrap_or(TITULO_AUTOMATICO_MAXIMO)
    };
    let inicio: String = caracteres[..corte].iter().collect();
    let inicio = inicio.trim_end_matches(|c: char| c.is_whitespace() || ",;:-–—".contains(c));
    format!("{inicio}…")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mensagens_curtas_viram_o_titulo_como_estao() {
        assert_eq!(titulo_da_mensagem("Oi, tudo bem?"), "Oi, tudo bem?");
        assert_eq!(titulo_da_mensagem("  Olá   mundo  "), "Olá mundo");
    }

    #[test]
    fn usa_so_a_primeira_linha_com_texto() {
        assert_eq!(
            titulo_da_mensagem("\n\n  Primeira linha\nSegunda linha"),
            "Primeira linha"
        );
    }

    #[test]
    fn mensagem_vazia_ou_em_branco_mantem_o_titulo_padrao() {
        assert_eq!(titulo_da_mensagem(""), TITULO_PADRAO_DA_CONVERSA);
        assert_eq!(titulo_da_mensagem(" \n\t "), TITULO_PADRAO_DA_CONVERSA);
    }

    #[test]
    fn corta_em_40_caracteres_no_fim_de_palavra_com_reticencias() {
        let titulo =
            titulo_da_mensagem("Explique como funciona o ciclo de vida de um processo no Windows");

        assert_eq!(titulo, "Explique como funciona o ciclo de vida…");
        assert!(titulo.chars().count() <= TITULO_AUTOMATICO_MAXIMO + 1);
    }

    #[test]
    fn exatamente_40_caracteres_nao_leva_reticencias() {
        let quarenta = "a".repeat(40);

        assert_eq!(titulo_da_mensagem(&quarenta), quarenta);
    }

    #[test]
    fn corte_limpo_quando_a_palavra_termina_no_limite() {
        let texto = format!("{} depois", "x".repeat(40));

        assert_eq!(titulo_da_mensagem(&texto), format!("{}…", "x".repeat(40)));
    }

    #[test]
    fn palavra_sem_espacos_maior_que_o_limite_e_cortada_no_limite() {
        let titulo = titulo_da_mensagem(&"z".repeat(100));

        assert_eq!(titulo, format!("{}…", "z".repeat(40)));
    }

    #[test]
    fn nao_deixa_pontuacao_pendurada_antes_da_reticencia() {
        let titulo =
            titulo_da_mensagem("Quais são as diferenças entre Rust e Go, em termos de desempenho?");

        assert!(titulo.ends_with('…'));
        assert!(!titulo.contains(",…"));
    }

    #[test]
    fn conta_caracteres_e_nao_bytes() {
        let titulo = titulo_da_mensagem(&"ação ".repeat(20));

        assert!(titulo.chars().count() <= TITULO_AUTOMATICO_MAXIMO + 1);
        assert!(titulo.ends_with('…'));
        assert!(!titulo.contains('\u{fffd}'));
    }
}
