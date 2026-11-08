//! Erros da instalação do Ollama.
//!
//! O texto técnico (`Display`) serve só para o log. O que o usuário vê é sempre
//! `mensagem_amigavel`: sem códigos de saída, caminhos, hashes nem texto de API.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroInstalacao {
    /// Já existe um Ollama instalado. O Memoteca nunca instala por cima.
    JaInstalado,
    /// Sem conexão com a internet (ou o servidor não foi alcançado).
    SemInternet,
    /// O servidor respondeu, mas não como esperado (erro HTTP, limite de consultas...).
    ServidorIndisponivel,
    /// A informação da release não serve (sem instalador, sem hash, origem não permitida).
    ReleaseInvalida(String),
    /// Não há espaço livre suficiente. Bloqueia antes de baixar.
    SemEspaco {
        necessario_em_bytes: u64,
        disponivel_em_bytes: u64,
    },
    /// A conexão caiu no meio do download.
    DownloadInterrompido,
    /// Nenhum byte chegou dentro do limite de ociosidade, ou a instalação passou do prazo.
    TempoEsgotado,
    /// O arquivo baixado não tem o tamanho ou o SHA-256 que a release informa.
    InstaladorCorrompido(String),
    /// A assinatura digital não é válida ou não é da Ollama Inc.
    AssinaturaInvalida(String),
    /// O Windows exigiu permissão de administrador, que o Memoteca nunca solicita.
    ElevacaoNecessaria,
    /// O instalador terminou com erro.
    InstaladorFalhou(i32),
    /// O instalador disse que terminou, mas o Ollama não apareceu.
    InstalacaoNaoDetectada,
    /// Não foi possível executar o instalador.
    NaoFoiPossivelExecutar(String),
    /// Falha inesperada (disco, pasta temporária...).
    Interno(String),
}

fn em_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_073_741_824.0).replace('.', ",")
}

impl ErroInstalacao {
    /// Mensagem em pt-BR para mostrar ao usuário.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::JaInstalado => {
                "O Ollama já está instalado neste computador, então não há nada a instalar."
                    .to_owned()
            }
            Self::SemInternet => "Não foi possível baixar o instalador do Ollama. Verifique sua \
                conexão com a internet e tente de novo."
                .to_owned(),
            Self::ServidorIndisponivel => "O servidor de download do Ollama não respondeu como \
                esperado. Tente de novo mais tarde."
                .to_owned(),
            Self::ReleaseInvalida(_) => "Não foi possível encontrar um instalador válido do \
                Ollama agora. Tente de novo mais tarde."
                .to_owned(),
            Self::SemEspaco {
                necessario_em_bytes,
                disponivel_em_bytes,
            } => format!(
                "Não há espaço livre suficiente para instalar o Ollama. São necessários cerca \
                 de {} GB livres e há {} GB. Libere espaço e tente de novo.",
                em_gb(*necessario_em_bytes),
                em_gb(*disponivel_em_bytes)
            ),
            Self::DownloadInterrompido => {
                "O download do instalador foi interrompido. Tente de novo.".to_owned()
            }
            Self::TempoEsgotado => {
                "A operação demorou demais e foi interrompida. Tente de novo.".to_owned()
            }
            Self::InstaladorCorrompido(_) => "O arquivo baixado está incompleto ou danificado, \
                então não foi executado. Tente de novo."
                .to_owned(),
            Self::AssinaturaInvalida(_) => "Não foi possível confirmar que o instalador veio \
                do Ollama, então ele não foi executado. Tente de novo mais tarde."
                .to_owned(),
            Self::ElevacaoNecessaria => "O Windows exigiu permissão de administrador para \
                instalar o Ollama, e o Memoteca não a solicita. Instale o Ollama pelo site \
                oficial (ollama.com) e abra o Memoteca de novo."
                .to_owned(),
            Self::InstaladorFalhou(_) | Self::InstalacaoNaoDetectada => {
                "A instalação do Ollama não foi concluída. Tente de novo.".to_owned()
            }
            Self::NaoFoiPossivelExecutar(_) => {
                "Não foi possível executar o instalador do Ollama. Tente de novo.".to_owned()
            }
            Self::Interno(_) => {
                "Ocorreu um problema inesperado durante a instalação. Tente de novo.".to_owned()
            }
        }
    }
}

impl fmt::Display for ErroInstalacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JaInstalado => write!(f, "o Ollama já está instalado"),
            Self::SemInternet => write!(f, "sem conexão com o servidor"),
            Self::ServidorIndisponivel => write!(f, "o servidor respondeu com erro"),
            Self::ReleaseInvalida(detalhe) => write!(f, "release inválida: {detalhe}"),
            Self::SemEspaco {
                necessario_em_bytes,
                disponivel_em_bytes,
            } => write!(
                f,
                "sem espaço: precisa de {necessario_em_bytes} bytes e há {disponivel_em_bytes}"
            ),
            Self::DownloadInterrompido => write!(f, "download interrompido"),
            Self::TempoEsgotado => write!(f, "tempo esgotado"),
            Self::InstaladorCorrompido(detalhe) => write!(f, "instalador corrompido: {detalhe}"),
            Self::AssinaturaInvalida(detalhe) => write!(f, "assinatura inválida: {detalhe}"),
            Self::ElevacaoNecessaria => write!(f, "o instalador exigiu elevação"),
            Self::InstaladorFalhou(codigo) => {
                write!(f, "o instalador terminou com o código {codigo}")
            }
            Self::InstalacaoNaoDetectada => write!(f, "o Ollama não apareceu após a instalação"),
            Self::NaoFoiPossivelExecutar(detalhe) => {
                write!(f, "não foi possível executar o instalador: {detalhe}")
            }
            Self::Interno(detalhe) => write!(f, "erro interno: {detalhe}"),
        }
    }
}

impl std::error::Error for ErroInstalacao {}

/// Falha de uma instalação, com a situação da pasta de instalação depois do rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalhaInstalacao {
    pub erro: ErroInstalacao,
    /// Sobraram arquivos de uma instalação parcial que não foi possível remover.
    pub restos_na_instalacao: bool,
}

impl FalhaInstalacao {
    pub fn mensagem_amigavel(&self) -> String {
        let mut texto = self.erro.mensagem_amigavel();
        if self.restos_na_instalacao {
            texto.push_str(
                " Alguns arquivos da tentativa anterior não puderam ser removidos. Feche o \
                 Ollama, se ele estiver aberto, e tente de novo.",
            );
        }
        texto
    }
}

impl fmt::Display for FalhaInstalacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.erro)
    }
}

impl std::error::Error for FalhaInstalacao {}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formata_gigabytes_com_virgula() {
        assert_eq!(em_gb(1_073_741_824), "1,0");
        assert_eq!(em_gb(6_000_000_000), "5,6");
    }

    #[test]
    fn a_mensagem_de_espaco_informa_necessario_e_disponivel() {
        let mensagem = ErroInstalacao::SemEspaco {
            necessario_em_bytes: 6_000_000_000,
            disponivel_em_bytes: 1_073_741_824,
        }
        .mensagem_amigavel();

        assert!(mensagem.contains("5,6 GB") && mensagem.contains("1,0 GB"));
    }

    #[test]
    fn restos_da_instalacao_entram_na_mensagem() {
        let falha = FalhaInstalacao {
            erro: ErroInstalacao::InstaladorFalhou(4),
            restos_na_instalacao: true,
        };

        assert!(
            falha
                .mensagem_amigavel()
                .contains("não puderam ser removidos")
        );
        assert!(!falha.mensagem_amigavel().contains('4'));
    }
}
