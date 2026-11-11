//! Catálogo das mensagens de erro que o usuário pode ver (E14).
//!
//! Os textos ficam com cada tipo de erro (`mensagem_amigavel`), junto de quem sabe o que
//! aconteceu. Este módulo **lista todas** num lugar só e define as regras que toda mensagem
//! precisa cumprir. Um teste confere cada uma, então nenhuma mensagem nova (nem alterada)
//! escapa das regras:
//!
//! * está em português, é uma frase completa e termina com pontuação;
//! * diz o que o usuário pode fazer, ou explica que a ação não precisa dele;
//! * **nunca** traz texto técnico: SQL, HTTP, JSON, caminhos, códigos de saída, endereços,
//!   nomes de tipos ou o detalhe que só vai para o log.
//!
//! Ao criar uma variante de erro, o `match` exaustivo de `confere_que_nenhuma_variante_ficou_de_fora`
//! deixa de compilar até ela ser incluída aqui.

use crate::banco::intercambio::ErroDeBackup;
use crate::banco::{ErroBanco, MEMORIA_MAXIMA};
use crate::conversas::{ErroGeracao, ErroSugestao};
use crate::instalador::erro::{ErroInstalacao, FalhaInstalacao};
use crate::intercambio::ErroDeExportacao;
use crate::modelos::{ErroModelo, ErroNome};
use crate::ollama::ciclo::EstadoOllama;
use crate::ollama::{ErroOllama, VERSAO_MINIMA, Versao};

/// O texto que só pode ir para o log. Se aparecer numa mensagem, o teste falha.
pub const DETALHE_TECNICO: &str = "DETALHE-TECNICO-SO-PARA-O-LOG";

/// Uma mensagem do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// De onde vem (tipo e variante), para o teste dizer qual mensagem está errada.
    pub origem: String,
    pub texto: String,
    /// A mensagem só informa (não há o que o usuário fazer), então não precisa dizer o que fazer.
    pub informativa: bool,
}

fn entrada(origem: &str, texto: String) -> Entrada {
    Entrada {
        origem: origem.to_owned(),
        texto,
        informativa: false,
    }
}

fn informativa(origem: &str, texto: String) -> Entrada {
    Entrada {
        informativa: true,
        ..entrada(origem, texto)
    }
}

/// Todas as variantes de `ErroOllama`, com o detalhe técnico preenchido.
fn erros_do_ollama() -> Vec<ErroOllama> {
    vec![
        ErroOllama::OllamaForaDoAr,
        ErroOllama::PortaOcupada,
        ErroOllama::VersaoAntiga {
            encontrada: Versao::nova(0, 29, 9),
            minima: VERSAO_MINIMA,
        },
        ErroOllama::ModeloNaoEncontrado,
        ErroOllama::ModeloInexistente,
        ErroOllama::ModeloSemSuporte,
        ErroOllama::TempoEsgotado,
        ErroOllama::ConexaoInterrompida,
        ErroOllama::RespostaInvalida(DETALHE_TECNICO.into()),
        ErroOllama::ErroDoOllama(DETALHE_TECNICO.into()),
        ErroOllama::FalhaAoIniciar(DETALHE_TECNICO.into()),
    ]
}

fn erros_do_banco() -> Vec<ErroBanco> {
    vec![
        ErroBanco::Indisponivel(DETALHE_TECNICO.into()),
        ErroBanco::SemEspaco,
        ErroBanco::MigracaoFalhou {
            versao: 2,
            detalhe: DETALHE_TECNICO.into(),
            copia_guardada: false,
        },
        ErroBanco::MigracaoFalhou {
            versao: 2,
            detalhe: DETALHE_TECNICO.into(),
            copia_guardada: true,
        },
        ErroBanco::VersaoMaisNova {
            encontrada: 9,
            suportada: 2,
        },
        ErroBanco::NaoEncontrado,
        ErroBanco::Invalido(DETALHE_TECNICO.into()),
        ErroBanco::Interno(DETALHE_TECNICO.into()),
    ]
}

fn erros_da_instalacao() -> Vec<ErroInstalacao> {
    vec![
        ErroInstalacao::JaInstalado,
        ErroInstalacao::SemInternet,
        ErroInstalacao::ServidorIndisponivel,
        ErroInstalacao::ReleaseInvalida(DETALHE_TECNICO.into()),
        ErroInstalacao::SemEspaco {
            necessario_em_bytes: 4 * 1024 * 1024 * 1024,
            disponivel_em_bytes: 1024 * 1024 * 1024,
        },
        ErroInstalacao::DownloadInterrompido,
        ErroInstalacao::TempoEsgotado,
        ErroInstalacao::InstaladorCorrompido(DETALHE_TECNICO.into()),
        ErroInstalacao::AssinaturaInvalida(DETALHE_TECNICO.into()),
        ErroInstalacao::ElevacaoNecessaria,
        ErroInstalacao::InstaladorFalhou(1223),
        ErroInstalacao::InstalacaoNaoDetectada,
        ErroInstalacao::NaoFoiPossivelExecutar(DETALHE_TECNICO.into()),
        ErroInstalacao::Interno(DETALHE_TECNICO.into()),
    ]
}

/// Todas as mensagens de erro que o usuário pode ver.
pub fn todas() -> Vec<Entrada> {
    let mut lista = Vec::new();

    for erro in erros_do_ollama() {
        lista.push(entrada(
            &format!("ErroOllama::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }
    for erro in erros_do_banco() {
        lista.push(entrada(
            &format!("ErroBanco::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }
    // `Rejeitado` traz a mensagem pronta de quem o criou (um limite que o usuário passou).
    lista.push(entrada(
        "ErroBanco::Rejeitado",
        ErroBanco::Rejeitado("Dê um nome ao projeto.".into()).mensagem_amigavel(),
    ));
    lista.push(entrada(
        "ErroBanco::Rejeitado (memória)",
        ErroBanco::Rejeitado(format!(
            "Uma memória pode ter no máximo {MEMORIA_MAXIMA} caracteres. Resuma o texto."
        ))
        .mensagem_amigavel(),
    ));

    for erro in erros_da_instalacao() {
        let origem = format!("ErroInstalacao::{erro:?}");
        // "Já instalado" só informa: não há nada que o usuário precise fazer.
        let mensagem = if matches!(erro, ErroInstalacao::JaInstalado) {
            informativa(&origem, erro.mensagem_amigavel())
        } else {
            entrada(&origem, erro.mensagem_amigavel())
        };
        lista.push(mensagem);
        lista.push(entrada(
            &format!("FalhaInstalacao(restos)::{erro:?}"),
            FalhaInstalacao {
                erro,
                restos_na_instalacao: true,
            }
            .mensagem_amigavel(),
        ));
    }

    for erro in [ErroNome::Vazio, ErroNome::Invalido] {
        lista.push(entrada(
            &format!("ErroNome::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }
    for erro in [
        ErroModelo::Nome(ErroNome::Invalido),
        ErroModelo::SemEspaco {
            necessario_em_bytes: 8 * 1024 * 1024 * 1024,
            disponivel_em_bytes: 1024 * 1024 * 1024,
        },
        ErroModelo::Ollama(ErroOllama::ConexaoInterrompida),
        ErroModelo::Ollama(ErroOllama::ErroDoOllama(DETALHE_TECNICO.into())),
        ErroModelo::NaoApareceuNaLista,
    ] {
        lista.push(entrada(
            &format!("ErroModelo::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }

    for erro in [
        ErroGeracao::JaGerando,
        ErroGeracao::SemModelo,
        ErroGeracao::NadaParaResponder,
        ErroGeracao::NadaParaRegenerar,
        ErroGeracao::RespostaVazia,
        ErroGeracao::Banco(ErroBanco::Interno(DETALHE_TECNICO.into())),
        ErroGeracao::Ollama(ErroOllama::ErroDoOllama(DETALHE_TECNICO.into())),
    ] {
        lista.push(entrada(
            &format!("ErroGeracao::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }

    for erro in [
        ErroSugestao::SemModelo,
        ErroSugestao::ConversaVazia,
        ErroSugestao::RespostaInvalida,
        ErroSugestao::Banco(ErroBanco::Interno(DETALHE_TECNICO.into())),
        ErroSugestao::Ollama(ErroOllama::ErroDoOllama(DETALHE_TECNICO.into())),
    ] {
        lista.push(entrada(
            &format!("ErroSugestao::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }

    for erro in [
        ErroDeBackup::ArquivoIlegivel(DETALHE_TECNICO.into()),
        ErroDeBackup::ArquivoGrandeDemais,
        ErroDeBackup::NaoEBackup(DETALHE_TECNICO.into()),
        ErroDeBackup::VersaoDesconhecida(Some(7)),
        ErroDeBackup::VersaoDesconhecida(None),
        ErroDeBackup::Invalido(DETALHE_TECNICO.into()),
        ErroDeBackup::Banco(ErroBanco::Interno(DETALHE_TECNICO.into())),
    ] {
        lista.push(entrada(
            &format!("ErroDeBackup::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }

    for erro in [
        ErroDeExportacao::SemPermissao,
        ErroDeExportacao::SemEspaco,
        ErroDeExportacao::DestinoInvalido,
        ErroDeExportacao::Banco(ErroBanco::Interno(DETALHE_TECNICO.into())),
        ErroDeExportacao::Outro(DETALHE_TECNICO.into()),
    ] {
        lista.push(entrada(
            &format!("ErroDeExportacao::{erro:?}"),
            erro.mensagem_amigavel(),
        ));
    }

    // O que a tela recebe do estado do Ollama na inicialização.
    for estado in [
        EstadoOllama::NaoInstalado,
        EstadoOllama::PortaOcupada,
        EstadoOllama::Indisponivel(ErroOllama::FalhaAoIniciar(DETALHE_TECNICO.into())),
    ] {
        if let Some(texto) = estado.para_interface().mensagem {
            lista.push(informativa(&format!("EstadoOllama::{estado:?}"), texto));
        }
    }

    lista
}

/// Garante, em tempo de compilação, que nenhuma variante dos erros acima foi esquecida: se
/// uma variante nova for criada, este `match` deixa de compilar até ela entrar na lista.
#[allow(dead_code)]
fn confere_que_nenhuma_variante_ficou_de_fora(
    ollama: &ErroOllama,
    banco: &ErroBanco,
    instalacao: &ErroInstalacao,
    geracao: &ErroGeracao,
) {
    match ollama {
        ErroOllama::OllamaForaDoAr
        | ErroOllama::PortaOcupada
        | ErroOllama::VersaoAntiga { .. }
        | ErroOllama::ModeloNaoEncontrado
        | ErroOllama::ModeloInexistente
        | ErroOllama::ModeloSemSuporte
        | ErroOllama::TempoEsgotado
        | ErroOllama::ConexaoInterrompida
        | ErroOllama::RespostaInvalida(_)
        | ErroOllama::ErroDoOllama(_)
        | ErroOllama::FalhaAoIniciar(_) => {}
    }
    match banco {
        ErroBanco::Indisponivel(_)
        | ErroBanco::SemEspaco
        | ErroBanco::MigracaoFalhou { .. }
        | ErroBanco::VersaoMaisNova { .. }
        | ErroBanco::NaoEncontrado
        | ErroBanco::Invalido(_)
        | ErroBanco::Rejeitado(_)
        | ErroBanco::Interno(_) => {}
    }
    match instalacao {
        ErroInstalacao::JaInstalado
        | ErroInstalacao::SemInternet
        | ErroInstalacao::ServidorIndisponivel
        | ErroInstalacao::ReleaseInvalida(_)
        | ErroInstalacao::SemEspaco { .. }
        | ErroInstalacao::DownloadInterrompido
        | ErroInstalacao::TempoEsgotado
        | ErroInstalacao::InstaladorCorrompido(_)
        | ErroInstalacao::AssinaturaInvalida(_)
        | ErroInstalacao::ElevacaoNecessaria
        | ErroInstalacao::InstaladorFalhou(_)
        | ErroInstalacao::InstalacaoNaoDetectada
        | ErroInstalacao::NaoFoiPossivelExecutar(_)
        | ErroInstalacao::Interno(_) => {}
    }
    match geracao {
        ErroGeracao::JaGerando
        | ErroGeracao::SemModelo
        | ErroGeracao::NadaParaResponder
        | ErroGeracao::NadaParaRegenerar
        | ErroGeracao::RespostaVazia
        | ErroGeracao::Banco(_)
        | ErroGeracao::Ollama(_) => {}
    }
}

#[allow(dead_code)]
fn confere_que_nenhuma_variante_ficou_de_fora_2(
    sugestao: &ErroSugestao,
    backup: &ErroDeBackup,
    exportacao: &ErroDeExportacao,
    modelo: &ErroModelo,
) {
    match sugestao {
        ErroSugestao::SemModelo
        | ErroSugestao::ConversaVazia
        | ErroSugestao::RespostaInvalida
        | ErroSugestao::Banco(_)
        | ErroSugestao::Ollama(_) => {}
    }
    match backup {
        ErroDeBackup::ArquivoIlegivel(_)
        | ErroDeBackup::ArquivoGrandeDemais
        | ErroDeBackup::NaoEBackup(_)
        | ErroDeBackup::VersaoDesconhecida(_)
        | ErroDeBackup::Invalido(_)
        | ErroDeBackup::Banco(_) => {}
    }
    match exportacao {
        ErroDeExportacao::SemPermissao
        | ErroDeExportacao::SemEspaco
        | ErroDeExportacao::DestinoInvalido
        | ErroDeExportacao::Banco(_)
        | ErroDeExportacao::Outro(_) => {}
    }
    match modelo {
        ErroModelo::Nome(_)
        | ErroModelo::SemEspaco { .. }
        | ErroModelo::Ollama(_)
        | ErroModelo::NaoApareceuNaLista => {}
    }
}

/// Trechos que denunciam texto técnico numa mensagem para o usuário.
const TRECHOS_TECNICOS: &[&str] = &[
    "SQLITE",
    "SQLite",
    "sqlite",
    "SQL",
    "HTTP",
    "JSON",
    "json",
    "Exception",
    "stack",
    "panic",
    "unwrap",
    "errno",
    "0x",
    "::",
    "{",
    "}",
    "<",
    ">",
    "\\",
    "/api",
    "127.0.0.1",
    "localhost",
    "http://",
    "https://",
    "SHA",
    "hash",
    "digest",
    "Mutex",
    "tokio",
    "reqwest",
    "serde",
    "Some(",
    "Err(",
    "null",
    "NULL",
    "timeout",
    "socket",
    "backtrace",
    "TLS",
    "UTF-8",
];

/// Palavras em inglês que não devem aparecer numa mensagem em português.
const PALAVRAS_EM_INGLES: &[&str] = &[
    " the ",
    " failed",
    " error",
    "cannot ",
    "unable ",
    " invalid",
    "denied",
    "not found",
    "please ",
];

/// Verbos que mostram o que o usuário pode fazer.
const ACOES: &[&str] = &[
    "Tente",
    "Escolha",
    "Feche",
    "Abra",
    "Atualize",
    "Libere",
    "Verifique",
    "Confira",
    "Baixe",
    "Reinicie",
    "Instale",
    "Resuma",
    "Dê ",
    "Divida",
    "Converse",
    "Espere",
    "Aguarde",
    "Digite",
    "Use ",
    "Escreva",
    "Envie",
    "Clique",
    "Procure",
    "Aumente",
    "Reduza",
    "Corrija",
    "Instale",
];

/// Os problemas de uma mensagem. Vazio quer dizer que ela cumpre as regras.
pub fn problemas(mensagem: &Entrada) -> Vec<String> {
    let texto = mensagem.texto.as_str();
    let mut achados = Vec::new();

    if texto.trim().is_empty() {
        return vec!["mensagem vazia".to_owned()];
    }
    if texto.chars().count() > 450 {
        achados.push("longa demais: perde o usuário antes do fim".to_owned());
    }
    if texto != texto.trim() || texto.contains('\n') || texto.contains("  ") {
        achados.push("espaços ou quebras de linha sobrando".to_owned());
    }
    if !texto.chars().next().is_some_and(char::is_uppercase) {
        achados.push("não começa com letra maiúscula".to_owned());
    }
    if !texto.ends_with(['.', '?', '!']) {
        achados.push("não termina com pontuação".to_owned());
    }
    if texto.contains(DETALHE_TECNICO) {
        achados.push("deixou passar o detalhe técnico, que é só para o log".to_owned());
    }
    for trecho in TRECHOS_TECNICOS {
        if texto.contains(trecho) {
            achados.push(format!("texto técnico: {trecho:?}"));
        }
    }
    let minuscula = texto.to_lowercase();
    for palavra in PALAVRAS_EM_INGLES {
        if minuscula.contains(palavra) {
            achados.push(format!("palavra em inglês: {palavra:?}"));
        }
    }
    if !mensagem.informativa && !ACOES.iter().any(|acao| texto.contains(acao)) {
        achados.push("não diz o que o usuário pode fazer".to_owned());
    }
    achados
}

#[cfg(test)]
mod testes {
    use super::*;

    fn entrada_de(texto: &str) -> Entrada {
        Entrada {
            origem: "teste".into(),
            texto: texto.into(),
            informativa: false,
        }
    }

    #[test]
    fn as_regras_reconhecem_mensagens_boas_e_ruins() {
        assert!(problemas(&entrada_de("O Ollama não está respondendo. Tente de novo.")).is_empty());

        for (ruim, trecho) in [
            ("", "vazia"),
            ("falhou. Tente de novo.", "maiúscula"),
            ("O Ollama caiu. Tente de novo", "pontuação"),
            ("Erro de SQLITE no banco. Tente de novo.", "técnico"),
            (
                "A resposta veio em JSON inválido. Tente de novo.",
                "técnico",
            ),
            (
                "O arquivo C:\\Users\\a\\b.db falhou. Tente de novo.",
                "técnico",
            ),
            ("The request failed. Tente de novo.", "inglês"),
            ("O Ollama caiu.", "o que o usuário pode fazer"),
            ("Algo deu errado.\nTente de novo.", "quebras"),
        ] {
            let achados = problemas(&entrada_de(ruim));
            assert!(
                achados.iter().any(|a| a.contains(trecho)),
                "{ruim:?} deveria ser recusada por {trecho:?}, mas: {achados:?}"
            );
        }
    }

    #[test]
    fn mensagem_so_informativa_nao_precisa_dizer_o_que_fazer() {
        let mut mensagem = entrada_de("O Ollama não está instalado neste computador.");
        assert!(!problemas(&mensagem).is_empty());
        mensagem.informativa = true;
        assert!(problemas(&mensagem).is_empty());
    }

    #[test]
    fn todas_as_mensagens_do_catalogo_cumprem_as_regras() {
        let mensagens = todas();
        assert!(
            mensagens.len() > 80,
            "o catálogo encolheu: {}",
            mensagens.len()
        );

        let mut recusadas = Vec::new();
        for mensagem in &mensagens {
            let achados = problemas(mensagem);
            if !achados.is_empty() {
                recusadas.push(format!(
                    "{}: {:?}\n    -> {}",
                    mensagem.origem,
                    mensagem.texto,
                    achados.join("; ")
                ));
            }
        }
        assert!(
            recusadas.is_empty(),
            "{} mensagem(ns) fora das regras:\n{}",
            recusadas.len(),
            recusadas.join("\n")
        );
    }
}
