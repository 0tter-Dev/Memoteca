//! Regra de espaço em disco antes de baixar um modelo.
//!
//! * Bloqueia quando o espaço livre é **menor que o tamanho** do que falta baixar.
//! * Alerta (e deixa continuar, se o usuário quiser) quando depois do download sobrariam
//!   **menos de 5 GB** livres, para não deixar o disco no limite.
//! * Se o espaço livre não puder ser lido, não bloqueia.
//!
//! A medição é na unidade da pasta de modelos do Ollama, que pode ser outra que não a do
//! sistema (`OLLAMA_MODELS`).

use std::ffi::OsString;
use std::path::PathBuf;

use serde::Serialize;

use crate::instalador::erro::em_gb;

/// Folga abaixo da qual o app alerta, mesmo cabendo o modelo.
pub const FOLGA_PARA_ALERTA_EM_BYTES: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvaliacaoDeEspaco {
    Suficiente,
    /// Cabe, mas sobra pouco.
    Alerta {
        sobra_em_bytes: u64,
    },
    /// Não cabe. O download é bloqueado.
    Insuficiente {
        necessario_em_bytes: u64,
        disponivel_em_bytes: u64,
    },
    /// Não foi possível medir (ou o tamanho do modelo ainda é desconhecido).
    Desconhecido,
}

/// Avalia se `necessario` bytes cabem em `livre`.
pub fn avaliar(necessario: u64, livre: Option<u64>) -> AvaliacaoDeEspaco {
    let Some(livre) = livre else {
        return AvaliacaoDeEspaco::Desconhecido;
    };
    if livre < necessario {
        return AvaliacaoDeEspaco::Insuficiente {
            necessario_em_bytes: necessario,
            disponivel_em_bytes: livre,
        };
    }
    let sobra = livre - necessario;
    if sobra < FOLGA_PARA_ALERTA_EM_BYTES {
        AvaliacaoDeEspaco::Alerta {
            sobra_em_bytes: sobra,
        }
    } else {
        AvaliacaoDeEspaco::Suficiente
    }
}

/// Pasta onde o Ollama guarda os modelos: `OLLAMA_MODELS`, ou `%USERPROFILE%\.ollama\models`.
/// É o melhor palpite possível, já que um Ollama externo pode ter outra configuração.
pub fn pasta_de_modelos(
    ollama_models: Option<OsString>,
    perfil_do_usuario: Option<OsString>,
) -> Option<PathBuf> {
    ollama_models
        .filter(|valor| !valor.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            perfil_do_usuario
                .filter(|valor| !valor.is_empty())
                .map(|perfil| PathBuf::from(perfil).join(".ollama").join("models"))
        })
}

/// A pasta de modelos no ambiente real.
pub fn pasta_de_modelos_do_ambiente() -> Option<PathBuf> {
    pasta_de_modelos(
        std::env::var_os("OLLAMA_MODELS"),
        std::env::var_os("USERPROFILE"),
    )
}

/// Avaliação em formato simples para a interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvaliacaoParaInterface {
    /// `suficiente`, `alerta`, `insuficiente` ou `desconhecido`.
    pub estado: &'static str,
    pub livre_em_bytes: Option<u64>,
    pub necessario_em_bytes: Option<u64>,
    /// Mensagem amigável quando há alerta ou bloqueio.
    pub mensagem: Option<String>,
}

impl AvaliacaoDeEspaco {
    pub fn para_interface(self, necessario: u64, livre: Option<u64>) -> AvaliacaoParaInterface {
        let (estado, mensagem) = match self {
            Self::Suficiente => ("suficiente", None),
            Self::Desconhecido => ("desconhecido", None),
            Self::Alerta { sobra_em_bytes } => (
                "alerta",
                Some(format!(
                    "Depois do download, sobrariam só {} GB livres no disco. Você pode baixar \
                     mesmo assim, mas o computador pode ficar sem espaço.",
                    em_gb(sobra_em_bytes)
                )),
            ),
            Self::Insuficiente {
                necessario_em_bytes,
                disponivel_em_bytes,
            } => (
                "insuficiente",
                Some(mensagem_de_espaco_insuficiente(
                    necessario_em_bytes,
                    disponivel_em_bytes,
                )),
            ),
        };
        AvaliacaoParaInterface {
            estado,
            livre_em_bytes: livre,
            necessario_em_bytes: Some(necessario),
            mensagem,
        }
    }
}

pub fn mensagem_de_espaco_insuficiente(necessario: u64, disponivel: u64) -> String {
    format!(
        "Não há espaço livre suficiente para baixar este modelo. São necessários {} GB e há {} \
         GB livres. Libere espaço e tente de novo.",
        em_gb(necessario),
        em_gb(disponivel)
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn bloqueia_quando_nao_cabe() {
        assert_eq!(
            avaliar(8 * GB, Some(7 * GB)),
            AvaliacaoDeEspaco::Insuficiente {
                necessario_em_bytes: 8 * GB,
                disponivel_em_bytes: 7 * GB
            }
        );
        assert!(matches!(
            avaliar(1, Some(0)),
            AvaliacaoDeEspaco::Insuficiente { .. }
        ));
    }

    #[test]
    fn alerta_quando_cabe_mas_sobram_menos_de_5_gb() {
        assert_eq!(
            avaliar(8 * GB, Some(8 * GB)),
            AvaliacaoDeEspaco::Alerta { sobra_em_bytes: 0 }
        );
        assert_eq!(
            avaliar(8 * GB, Some(12 * GB)),
            AvaliacaoDeEspaco::Alerta {
                sobra_em_bytes: 4 * GB
            }
        );
    }

    #[test]
    fn libera_quando_sobram_5_gb_ou_mais() {
        assert_eq!(
            avaliar(8 * GB, Some(13 * GB)),
            AvaliacaoDeEspaco::Suficiente
        );
        assert_eq!(
            avaliar(8 * GB, Some(100 * GB)),
            AvaliacaoDeEspaco::Suficiente
        );
    }

    #[test]
    fn sem_medida_nao_bloqueia() {
        assert_eq!(avaliar(8 * GB, None), AvaliacaoDeEspaco::Desconhecido);
    }

    #[test]
    fn a_pasta_de_modelos_segue_o_ollama_models_ou_o_padrao() {
        assert_eq!(
            pasta_de_modelos(Some(r"E:\Modelos".into()), Some(r"C:\Users\ana".into())),
            Some(PathBuf::from(r"E:\Modelos"))
        );
        assert_eq!(
            pasta_de_modelos(None, Some(r"C:\Users\ana".into())),
            Some(
                PathBuf::from(r"C:\Users\ana")
                    .join(".ollama")
                    .join("models")
            )
        );
        assert_eq!(
            pasta_de_modelos(Some("".into()), Some(r"C:\Users\ana".into())),
            Some(
                PathBuf::from(r"C:\Users\ana")
                    .join(".ollama")
                    .join("models")
            )
        );
        assert_eq!(pasta_de_modelos(None, None), None);
    }

    #[test]
    fn as_mensagens_informam_os_numeros_em_gb_sem_texto_tecnico() {
        let bloqueio = avaliar(8 * GB, Some(2 * GB)).para_interface(8 * GB, Some(2 * GB));
        assert_eq!(bloqueio.estado, "insuficiente");
        let mensagem = bloqueio.mensagem.unwrap();
        assert!(mensagem.contains("8,0 GB") && mensagem.contains("2,0 GB"));

        let alerta = avaliar(8 * GB, Some(10 * GB)).para_interface(8 * GB, Some(10 * GB));
        assert_eq!(alerta.estado, "alerta");
        assert!(alerta.mensagem.unwrap().contains("2,0 GB"));

        let livre = avaliar(8 * GB, Some(50 * GB)).para_interface(8 * GB, Some(50 * GB));
        assert_eq!((livre.estado, livre.mensagem), ("suficiente", None));
    }
}
