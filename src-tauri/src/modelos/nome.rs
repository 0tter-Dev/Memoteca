//! Validação do nome de um modelo digitado pelo usuário (ou vindo do catálogo).
//!
//! Formato aceito: `[host/][namespace/]modelo[:tag]`, só com letras, números, ponto, hífen e
//! sublinhado. Não aceita espaços, esquemas (`http://`), portas nem caminhos relativos. A
//! tag é opcional (o Ollama usa `latest`).

use std::fmt;

const TAMANHO_MAXIMO: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NomeDeModelo(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroNome {
    Vazio,
    Invalido,
}

impl ErroNome {
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::Vazio => "Digite o nome do modelo, por exemplo llama3.2:3b.".to_owned(),
            Self::Invalido => "O nome do modelo não é válido. Use o formato nome ou nome:tag, \
                por exemplo llama3.2:3b, sem espaços."
                .to_owned(),
        }
    }
}

impl fmt::Display for ErroNome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vazio => write!(f, "nome de modelo vazio"),
            Self::Invalido => write!(f, "nome de modelo inválido"),
        }
    }
}

impl std::error::Error for ErroNome {}

fn caractere_permitido(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')
}

/// Um trecho (`namespace`, `modelo` ou `tag`): só caracteres permitidos e começando por
/// letra ou número, o que também recusa `.` e `..`.
fn trecho_valido(trecho: &str) -> bool {
    trecho
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && trecho.chars().all(caractere_permitido)
}

impl NomeDeModelo {
    pub fn validar(texto: &str) -> Result<Self, ErroNome> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err(ErroNome::Vazio);
        }
        if texto.len() > TAMANHO_MAXIMO {
            return Err(ErroNome::Invalido);
        }

        let segmentos: Vec<&str> = texto.split('/').collect();
        if segmentos.len() > 3 {
            return Err(ErroNome::Invalido);
        }
        let (ultimo, anteriores) = segmentos.split_last().ok_or(ErroNome::Invalido)?;

        if !anteriores.iter().all(|segmento| trecho_valido(segmento)) {
            return Err(ErroNome::Invalido);
        }

        // O último segmento é `modelo` ou `modelo:tag`, com no máximo um `:`.
        let mut partes = ultimo.split(':');
        let modelo = partes.next().unwrap_or_default();
        let tag = partes.next();
        if partes.next().is_some()
            || !trecho_valido(modelo)
            || tag.is_some_and(|t| !trecho_valido(t))
        {
            return Err(ErroNome::Invalido);
        }
        Ok(Self(texto.to_owned()))
    }

    pub fn como_texto(&self) -> &str {
        &self.0
    }

    /// O nome como o Ollama o lista: com `:latest` quando a tag foi omitida.
    pub fn com_tag_padrao(&self) -> String {
        let ultimo = self.0.rsplit('/').next().unwrap_or(&self.0);
        if ultimo.contains(':') {
            self.0.clone()
        } else {
            format!("{}:latest", self.0)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aceita_nomes_comuns_do_ollama() {
        for nome in [
            "llama3.2",
            "llama3.2:3b",
            "qwen3:8b",
            "gpt-oss:20b",
            "library/llama3.2:latest",
            "usuario/meu-modelo:q4_K_M",
            "hf.co/usuario/modelo-gguf:Q4_K_M",
            "  llama3.2:3b  ",
            "deepseek-r1:1.5b",
        ] {
            assert!(NomeDeModelo::validar(nome).is_ok(), "{nome:?}");
        }
    }

    #[test]
    fn recusa_nomes_perigosos_ou_mal_formados() {
        for nome in [
            "com espaço",
            "llama3.2:",
            ":3b",
            "llama3.2:3b:extra",
            "http://exemplo.com/modelo",
            "../modelo",
            "a/../b",
            "./modelo",
            "a/b/c/d",
            "-modelo",
            "modelo;rm",
            "modelo\nnova linha",
            "modelo?x=1",
            "localhost:5000/ns/modelo",
            "modelo/",
            "/modelo",
            "acentuação",
        ] {
            assert_eq!(
                NomeDeModelo::validar(nome),
                Err(ErroNome::Invalido),
                "{nome:?}"
            );
        }
        assert_eq!(
            NomeDeModelo::validar(&"a".repeat(200)),
            Err(ErroNome::Invalido)
        );
    }

    #[test]
    fn nome_vazio_tem_erro_proprio() {
        assert_eq!(NomeDeModelo::validar(""), Err(ErroNome::Vazio));
        assert_eq!(NomeDeModelo::validar("   "), Err(ErroNome::Vazio));
    }

    #[test]
    fn normaliza_a_tag_padrao_como_o_ollama_lista() {
        let sem_tag = NomeDeModelo::validar("llama3.2").unwrap();
        let com_tag = NomeDeModelo::validar("llama3.2:3b").unwrap();
        let com_namespace = NomeDeModelo::validar("usuario/modelo").unwrap();

        assert_eq!(sem_tag.com_tag_padrao(), "llama3.2:latest");
        assert_eq!(com_tag.com_tag_padrao(), "llama3.2:3b");
        assert_eq!(com_namespace.com_tag_padrao(), "usuario/modelo:latest");
    }

    #[test]
    fn a_mensagem_amigavel_nao_tem_texto_tecnico() {
        for erro in [ErroNome::Vazio, ErroNome::Invalido] {
            let mensagem = erro.mensagem_amigavel();
            assert!(mensagem.contains("llama3.2:3b"));
            assert!(!mensagem.to_lowercase().contains("regex"));
        }
    }
}
