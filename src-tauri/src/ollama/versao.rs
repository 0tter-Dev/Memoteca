//! Versão do Ollama: leitura tolerante e comparação por número.

use std::fmt;

/// Versão mínima do Ollama aceita pelo Memoteca (ver o Registro de decisões).
pub const VERSAO_MINIMA: Versao = Versao::nova(0, 30, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Versao {
    pub principal: u32,
    pub secundaria: u32,
    pub correcao: u32,
}

impl Versao {
    pub const fn nova(principal: u32, secundaria: u32, correcao: u32) -> Self {
        Self {
            principal,
            secundaria,
            correcao,
        }
    }

    /// Lê `major.minor.patch`, ignorando um `v` inicial e sufixos como `-rc1` ou `+build`.
    /// Qualquer outro formato dá `None`.
    pub fn ler(texto: &str) -> Option<Self> {
        let texto = texto.trim();
        let texto = texto.strip_prefix('v').unwrap_or(texto);
        let numeros = texto.split(['-', '+']).next()?;

        let mut partes = numeros.split('.');
        let principal = partes.next()?.parse().ok()?;
        let secundaria = partes.next()?.parse().ok()?;
        let correcao = partes.next()?.parse().ok()?;
        if partes.next().is_some() {
            return None;
        }
        Some(Self::nova(principal, secundaria, correcao))
    }
}

impl fmt::Display for Versao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}",
            self.principal, self.secundaria, self.correcao
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_versoes_comuns_e_ignora_sufixos() {
        assert_eq!(Versao::ler("0.30.0"), Some(Versao::nova(0, 30, 0)));
        assert_eq!(Versao::ler("v0.40.15"), Some(Versao::nova(0, 40, 15)));
        assert_eq!(Versao::ler("0.31.0-rc1"), Some(Versao::nova(0, 31, 0)));
        assert_eq!(Versao::ler("1.2.3+build.7"), Some(Versao::nova(1, 2, 3)));
        assert_eq!(Versao::ler(" 0.9.0 "), Some(Versao::nova(0, 9, 0)));
    }

    #[test]
    fn formatos_invalidos_dao_none() {
        for texto in ["", "abc", "1.2", "1.2.3.4", "1.x.3", "-1.2.3", "0.30"] {
            assert_eq!(Versao::ler(texto), None, "{texto:?}");
        }
    }

    #[test]
    fn compara_por_numero_e_nao_por_texto() {
        assert!(Versao::nova(0, 9, 0) < VERSAO_MINIMA);
        assert!(Versao::nova(0, 29, 9) < VERSAO_MINIMA);
        assert!(Versao::nova(0, 30, 0) >= VERSAO_MINIMA);
        assert!(Versao::nova(0, 100, 0) > Versao::nova(0, 30, 0));
        assert!(Versao::nova(1, 0, 0) > Versao::nova(0, 99, 99));
    }

    #[test]
    fn mostra_a_versao_como_texto() {
        assert_eq!(Versao::nova(0, 30, 1).to_string(), "0.30.1");
    }
}
