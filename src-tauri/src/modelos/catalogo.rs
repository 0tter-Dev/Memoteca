//! Catálogo curado de modelos, embutido no app (JSON, sem backend).
//!
//! `validar_catalogo` é o "esquema" do catálogo: todo campo é obrigatório, campos
//! desconhecidos são recusados, cada tag precisa ser um nome de modelo válido, e não pode
//! haver tag repetida. Um teste valida o catálogo embutido com essas regras.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::nome::NomeDeModelo;

/// Catálogo embutido no executável.
const CATALOGO_EMBUTIDO: &str = include_str!("../../catalogo/modelos.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Faixa {
    Leve,
    Equilibrado,
    Potente,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeloDoCatalogo {
    /// Tag real do Ollama (`modelo:tag`).
    pub tag: String,
    /// Nome amigável, mostrado na tela.
    pub nome: String,
    /// Descrição curta, em pt-BR.
    pub descricao: String,
    pub faixa: Faixa,
    /// Tamanho do download, em bytes (soma das camadas no registro do Ollama).
    pub tamanho_em_bytes: u64,
    /// Memória (RAM ou VRAM) estimada para rodar, em GB. É uma estimativa.
    pub ram_estimada_em_gb: u32,
    /// O modelo emite `thinking` (raciocínio).
    pub raciocinio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalogo {
    pub versao: u32,
    pub atualizado_em: String,
    pub modelos: Vec<ModeloDoCatalogo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroCatalogo {
    /// O JSON não tem o formato esperado (campo faltando, tipo errado, campo desconhecido).
    FormatoInvalido(String),
    Vazio,
    TagRepetida(String),
    TagInvalida(String),
    CampoVazio {
        tag: String,
        campo: &'static str,
    },
    TamanhoZero(String),
}

impl fmt::Display for ErroCatalogo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatoInvalido(detalhe) => write!(f, "catálogo com formato inválido: {detalhe}"),
            Self::Vazio => write!(f, "o catálogo não tem nenhum modelo"),
            Self::TagRepetida(tag) => write!(f, "tag repetida no catálogo: {tag}"),
            Self::TagInvalida(tag) => write!(f, "tag inválida no catálogo: {tag}"),
            Self::CampoVazio { tag, campo } => write!(f, "o modelo {tag} está sem {campo}"),
            Self::TamanhoZero(tag) => write!(f, "o modelo {tag} tem tamanho zero"),
        }
    }
}

impl std::error::Error for ErroCatalogo {}

/// Lê e valida um catálogo em JSON.
pub fn validar_catalogo(json: &str) -> Result<Catalogo, ErroCatalogo> {
    let catalogo: Catalogo = serde_json::from_str(json)
        .map_err(|erro| ErroCatalogo::FormatoInvalido(erro.to_string()))?;

    if catalogo.modelos.is_empty() {
        return Err(ErroCatalogo::Vazio);
    }

    let mut vistas = HashSet::new();
    for modelo in &catalogo.modelos {
        if NomeDeModelo::validar(&modelo.tag).is_err() {
            return Err(ErroCatalogo::TagInvalida(modelo.tag.clone()));
        }
        // A comparação ignora maiúsculas: o Ollama trata `Llama3.2` e `llama3.2` como iguais.
        if !vistas.insert(modelo.tag.to_lowercase()) {
            return Err(ErroCatalogo::TagRepetida(modelo.tag.clone()));
        }
        for (campo, valor) in [("nome", &modelo.nome), ("descricao", &modelo.descricao)] {
            if valor.trim().is_empty() {
                return Err(ErroCatalogo::CampoVazio {
                    tag: modelo.tag.clone(),
                    campo,
                });
            }
        }
        if modelo.tamanho_em_bytes == 0 {
            return Err(ErroCatalogo::TamanhoZero(modelo.tag.clone()));
        }
    }
    Ok(catalogo)
}

/// O catálogo embutido, já validado. Um catálogo embutido inválido é erro de programação e
/// é pego pelos testes, então aqui só devolve `Err` em vez de derrubar o app.
pub fn catalogo_embutido() -> Result<Catalogo, ErroCatalogo> {
    validar_catalogo(CATALOGO_EMBUTIDO)
}

impl Catalogo {
    /// Procura um modelo pela tag, ignorando maiúsculas.
    pub fn buscar(&self, tag: &str) -> Option<&ModeloDoCatalogo> {
        self.modelos
            .iter()
            .find(|modelo| modelo.tag.eq_ignore_ascii_case(tag))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modelo(tag: &str) -> String {
        format!(
            r#"{{"tag":"{tag}","nome":"Nome","descricao":"Descrição","faixa":"leve",
                "tamanhoEmBytes":1000,"ramEstimadaEmGb":4,"raciocinio":false}}"#
        )
    }

    fn catalogo(modelos: &[String]) -> String {
        format!(
            r#"{{"versao":1,"atualizadoEm":"2026-11-08","modelos":[{}]}}"#,
            modelos.join(",")
        )
    }

    #[test]
    fn o_catalogo_embutido_passa_na_validacao() {
        let catalogo = catalogo_embutido().expect("o catálogo embutido deve ser válido");

        assert_eq!(catalogo.modelos.len(), 6);
        assert!(catalogo.buscar("qwen3:8b").is_some());
        assert!(catalogo.buscar("QWEN3:8B").is_some());
        assert!(catalogo.buscar("nao-existe:1b").is_none());
        for faixa in [Faixa::Leve, Faixa::Equilibrado, Faixa::Potente] {
            assert!(
                catalogo.modelos.iter().any(|m| m.faixa == faixa),
                "toda faixa deve ter ao menos um modelo"
            );
        }
        assert!(catalogo.modelos.iter().any(|m| m.raciocinio));
    }

    #[test]
    fn aceita_um_catalogo_correto() {
        assert!(validar_catalogo(&catalogo(&[modelo("llama3.2:3b")])).is_ok());
    }

    #[test]
    fn recusa_tag_repetida_mesmo_com_outra_caixa() {
        let json = catalogo(&[modelo("llama3.2:3b"), modelo("Llama3.2:3B")]);

        assert_eq!(
            validar_catalogo(&json),
            Err(ErroCatalogo::TagRepetida("Llama3.2:3B".into()))
        );
    }

    #[test]
    fn recusa_campo_obrigatorio_faltando() {
        for campo in [
            "tag",
            "nome",
            "descricao",
            "faixa",
            "tamanhoEmBytes",
            "ramEstimadaEmGb",
            "raciocinio",
        ] {
            let mut valor: serde_json::Value =
                serde_json::from_str(&modelo("llama3.2:3b")).unwrap();
            valor.as_object_mut().unwrap().remove(campo);
            let json = catalogo(&[valor.to_string()]);

            assert!(
                matches!(
                    validar_catalogo(&json),
                    Err(ErroCatalogo::FormatoInvalido(_))
                ),
                "sem o campo {campo}"
            );
        }
    }

    #[test]
    fn recusa_campo_desconhecido_e_tipo_errado() {
        let com_extra =
            modelo("llama3.2:3b").replace("\"raciocinio\"", "\"extra\":1,\"raciocinio\"");
        assert!(matches!(
            validar_catalogo(&catalogo(&[com_extra])),
            Err(ErroCatalogo::FormatoInvalido(_))
        ));

        let tamanho_texto = modelo("llama3.2:3b").replace("1000", "\"muito\"");
        assert!(matches!(
            validar_catalogo(&catalogo(&[tamanho_texto])),
            Err(ErroCatalogo::FormatoInvalido(_))
        ));

        let faixa_invalida = modelo("llama3.2:3b").replace("\"leve\"", "\"gigante\"");
        assert!(matches!(
            validar_catalogo(&catalogo(&[faixa_invalida])),
            Err(ErroCatalogo::FormatoInvalido(_))
        ));
    }

    #[test]
    fn recusa_catalogo_vazio_tag_invalida_texto_vazio_e_tamanho_zero() {
        assert_eq!(validar_catalogo(&catalogo(&[])), Err(ErroCatalogo::Vazio));
        assert_eq!(
            validar_catalogo(&catalogo(&[modelo("com espaço:1b")])),
            Err(ErroCatalogo::TagInvalida("com espaço:1b".into()))
        );
        assert!(matches!(
            validar_catalogo(&catalogo(&[modelo("a:1").replace("\"Nome\"", "\"  \"")])),
            Err(ErroCatalogo::CampoVazio { campo: "nome", .. })
        ));
        assert_eq!(
            validar_catalogo(&catalogo(&[modelo("a:1").replace("1000", "0")])),
            Err(ErroCatalogo::TamanhoZero("a:1".into()))
        );
        assert!(matches!(
            validar_catalogo("isto não é json"),
            Err(ErroCatalogo::FormatoInvalido(_))
        ));
    }
}
