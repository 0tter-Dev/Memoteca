//! Critérios de aceite da E16 que não precisam instalar nada: o aviso de independência em todos
//! os lugares, o nome "Memoteca" nos metadados e o workflow de release (tag, permissões,
//! rascunho). A parte que exige o instalador de verdade fica no workflow de release (que confere
//! o pacote gerado) e no checklist manual.

use serde_json::Value;

const AVISO_TS: &str = include_str!("../../src/avisoDeIndependencia.ts");
const README: &str = include_str!("../../README.md");
const PLANO: &str = include_str!("../../PLANO.md");
const NOTAS_DA_RELEASE: &str = include_str!("../../.github/notas-da-release.md");
const LICENCA_DO_INSTALADOR: &str = include_str!("../instalador/licenca.txt");
const TEXTOS_DO_INSTALADOR: &str = include_str!("../instalador/PortugueseBR.nsh");
const LICENSE: &str = include_str!("../../LICENSE");
const RELEASE: &str = include_str!("../../.github/workflows/release.yml");
const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CARGO: &str = include_str!("../Cargo.toml");
const PACKAGE_JSON: &str = include_str!("../../package.json");
const VALIDAR_TAG: &str = include_str!("../../scripts/validar-tag.mjs");
const VERIFICAR_PACOTE: &str = include_str!("../../scripts/verificar-pacote.mjs");

/// O aviso, como está em `src/avisoDeIndependencia.ts` (o texto da tela "Sobre").
fn aviso() -> String {
    let comeco = AVISO_TS
        .find("export const AVISO_DE_INDEPENDENCIA")
        .expect("constante do aviso");
    let resto = &AVISO_TS[comeco..];
    let inicio = resto.find('\'').expect("início do texto") + 1;
    let fim = resto[inicio..].find("';").expect("fim do texto") + inicio;
    resto[inicio..fim].to_owned()
}

fn sem_negrito(texto: &str) -> String {
    texto.replace("\r\n", "\n").replace("**", "")
}

/// A página de licença do instalador. É UTF-8: o empacotador acrescenta o BOM que o NSIS pede.
fn licenca_do_instalador() -> String {
    LICENCA_DO_INSTALADOR.replace("\r\n", "\n")
}

fn linhas_do_release() -> Vec<String> {
    RELEASE
        .lines()
        .map(|linha| match linha.find(" #") {
            Some(posicao) => &linha[..posicao],
            None => linha,
        })
        .map(str::trim_end)
        .filter(|linha| !linha.trim().is_empty() && !linha.trim_start().starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// As linhas de um job do release (do nome do job até o próximo job).
fn trecho_do_job(nome: &str) -> String {
    let linhas = linhas_do_release();
    let inicio = linhas
        .iter()
        .position(|l| l == &format!("  {nome}:"))
        .unwrap_or_else(|| panic!("o workflow de release deve ter o job `{nome}`"));
    linhas[inicio + 1..]
        .iter()
        .take_while(|l| !(l.starts_with("  ") && !l.starts_with("   ")))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

// --- aviso de independência (critério de falha) ------------------------------------------------

#[test]
fn o_aviso_de_independencia_e_o_mesmo_no_readme_na_tela_sobre_no_instalador_e_na_release() {
    let aviso = aviso();
    assert!(aviso.contains("não é afiliado, patrocinado, endossado nem mantido"));

    assert!(sem_negrito(README).contains(&aviso), "README.md");
    assert!(sem_negrito(PLANO).contains(&aviso), "PLANO.md (T09)");
    assert!(
        sem_negrito(NOTAS_DA_RELEASE).contains(&aviso),
        "notas da release"
    );
    assert!(
        licenca_do_instalador().contains(&aviso),
        "página de licença do instalador"
    );
}

#[test]
fn o_instalador_mostra_a_pagina_com_o_aviso_e_a_licenca_mit() {
    let conf: Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json válido");
    assert_eq!(
        conf["bundle"]["licenseFile"], "instalador/licenca.txt",
        "o instalador deve mostrar a página de licença com o aviso"
    );
    let licenca = licenca_do_instalador();
    assert!(licenca.contains(LICENSE.replace("\r\n", "\n").trim()));
}

// --- nome "Memoteca" e nenhuma sugestão de produto oficial (critério de falha) -----------------

#[test]
fn o_nome_nos_metadados_e_memoteca_e_o_nome_provisorio_nao_aparece() {
    let conf: Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json válido");
    assert_eq!(conf["productName"], "Memoteca");
    assert_eq!(conf["app"]["windows"][0]["title"], "Memoteca");
    assert_eq!(conf["bundle"]["publisher"], "Memoteca contributors");

    let nome_do_cargo = CARGO
        .lines()
        .find_map(|l| l.strip_prefix("name = "))
        .expect("nome do pacote");
    assert_eq!(nome_do_cargo, "\"memoteca\"");

    let licenca = licenca_do_instalador();
    for (arquivo, texto) in [
        ("tauri.conf.json", TAURI_CONF),
        ("Cargo.toml", CARGO),
        ("package.json", PACKAGE_JSON),
        ("release.yml", RELEASE),
        ("notas da release", NOTAS_DA_RELEASE),
        ("licença do instalador", licenca.as_str()),
        ("validar-tag.mjs", VALIDAR_TAG),
    ] {
        assert!(
            !texto.to_lowercase().contains("ollamaui"),
            "\"OllamaUI\" não pode aparecer em {arquivo}"
        );
    }
}

#[test]
fn nenhum_metadado_do_pacote_diz_que_o_app_e_do_ollama() {
    let conf: Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json válido");
    let bundle = &conf["bundle"];
    for campo in ["publisher", "copyright", "shortDescription"] {
        let texto = bundle[campo].as_str().unwrap_or_default().to_lowercase();
        assert!(
            !texto.contains("ollama"),
            "`{campo}` não pode citar o Ollama como autor ou dono"
        );
    }
    // A descrição longa só cita o Ollama para negar a relação.
    let longa = bundle["longDescription"].as_str().unwrap_or_default();
    if longa.to_lowercase().contains("ollama") {
        assert!(longa.contains("sem relação com o Ollama"));
    }
    let identificador = conf["identifier"].as_str().unwrap_or_default();
    assert!(!identificador.to_lowercase().contains("ollama"));
}

#[test]
fn o_instalador_e_nsis_por_usuario_em_portugues_e_o_script_de_conferencia_checa_o_nome() {
    let conf: Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json válido");
    assert_eq!(conf["bundle"]["targets"], serde_json::json!(["nsis"]));
    let nsis = &conf["bundle"]["windows"]["nsis"];
    assert_eq!(
        nsis["installMode"], "currentUser",
        "sem pedir administrador"
    );
    assert_eq!(nsis["languages"], serde_json::json!(["PortugueseBR"]));
    assert_eq!(nsis["displayLanguageSelector"], false);
    assert_eq!(
        nsis["customLanguageFiles"]["PortugueseBR"], "instalador/PortugueseBR.nsh",
        "os textos do instalador são os do projeto"
    );

    assert!(VERIFICAR_PACOTE.contains("ProductName"));
    assert!(VERIFICAR_PACOTE.contains("OllamaUI"));
    assert!(VERIFICAR_PACOTE.contains("MEMOTECA_E2E_DADOS"));
}

/// Política de dados na desinstalação (E16): a caixa de apagar os dados é a do desinstalador do
/// Tauri, que vem desmarcada. O texto diz o que será apagado, e nenhum texto do instalador
/// deixa `{{product_name}}` cru na tela.
#[test]
fn a_desinstalacao_mantem_os_dados_e_a_caixa_de_apagar_diz_o_que_some() {
    assert!(TEXTOS_DO_INSTALADOR.contains(
        "deleteAppData ${LANG_PORTUGUESEBR} \"Apagar também os meus dados (conversas, memórias e cópias de segurança)\""
    ));
    assert!(
        !TEXTOS_DO_INSTALADOR.contains("{{"),
        "modelo do Tauri sem preencher no texto do instalador"
    );
}

// --- workflow de release ------------------------------------------------------------------------

#[test]
fn a_release_so_dispara_por_tag_e_o_pull_request_so_gera_artefato() {
    let linhas = linhas_do_release();
    assert!(linhas.iter().any(|l| l == "  push:"));
    assert!(linhas.iter().any(|l| l.trim() == "tags: [\"v*\"]"));
    assert!(
        !linhas.iter().any(|l| l.trim() == "branches: [main]"),
        "um push na main não pode gerar release"
    );

    let texto = linhas.join("\n");
    assert!(!texto.contains("pull_request_target"));
    assert!(!texto.contains("workflow_run"));
    assert!(!texto.contains("workflow_dispatch"));

    // A criação da release fica só no job `publicar`, que só roda em tag.
    let publicar = trecho_do_job("publicar");
    assert!(publicar.contains("if: startsWith(github.ref, 'refs/tags/')"));
    let outros = format!(
        "{}\n{}",
        trecho_do_job("validar"),
        trecho_do_job("instalador")
    );
    assert!(
        !outros.contains("gh release"),
        "só o `publicar` cria release"
    );
}

#[test]
fn tag_invalida_para_no_primeiro_job_e_nao_gera_release() {
    let validar = trecho_do_job("validar");
    assert!(validar.contains("node scripts/validar-tag.mjs"));
    assert!(validar.contains("startsWith(github.ref, 'refs/tags/')"));

    // O instalador espera o `validar`: se ele falha, o instalador e a release são pulados.
    let instalador = trecho_do_job("instalador");
    assert!(instalador.contains("needs: validar"));
    assert!(instalador.contains("needs.validar.result == 'success'"));
    assert!(
        !instalador.contains("needs.validar.result == 'failure'"),
        "o instalador nunca pode rodar depois de um `validar` que falhou"
    );

    let publicar = trecho_do_job("publicar");
    assert!(publicar.contains("needs: instalador"));
}

#[test]
fn so_o_job_publicar_tem_escrita_e_nao_ha_segredos() {
    let linhas = linhas_do_release();
    let topo = linhas
        .iter()
        .position(|l| l == "permissions:")
        .expect("permissões no nível superior");
    assert_eq!(linhas[topo + 1], "  contents: read");

    let com_escrita: Vec<&String> = linhas.iter().filter(|l| l.contains(": write")).collect();
    assert_eq!(com_escrita.len(), 1, "só uma permissão de escrita");
    assert!(trecho_do_job("publicar").contains("contents: write"));

    let texto = linhas.join("\n");
    assert!(!texto.contains("write-all"));
    assert!(!texto.contains("secrets."), "sem assinatura, sem segredos");
    assert!(!texto.contains("${{ secrets"));
}

#[test]
fn a_release_e_um_rascunho_com_o_instalador_anexado_e_o_build_nao_liga_o_recurso_de_testes() {
    let publicar = trecho_do_job("publicar");
    assert!(publicar.contains("gh release create"));
    assert!(
        publicar.contains("--draft"),
        "o mantenedor publica depois de olhar"
    );
    assert!(publicar.contains("--verify-tag"));
    assert!(publicar.contains("instalador/*.exe") || publicar.contains("$arquivo"));

    let instalador = trecho_do_job("instalador");
    assert!(instalador.contains("run: pnpm tauri build"));
    assert!(
        !RELEASE.contains("e2e"),
        "o build de release nunca liga o recurso de testes"
    );
    assert!(instalador.contains("scripts/verificar-pacote.mjs"));
}

#[test]
fn todas_as_actions_do_release_estao_fixadas_por_commit() {
    let usos: Vec<String> = linhas_do_release()
        .iter()
        .filter_map(|l| {
            l.trim_start()
                .trim_start_matches("- ")
                .strip_prefix("uses:")
        })
        .map(|u| u.trim().to_owned())
        .collect();
    assert!(!usos.is_empty());
    for uso in usos {
        let (_, versao) = uso
            .split_once('@')
            .unwrap_or_else(|| panic!("action sem versão fixada: {uso}"));
        assert!(
            versao.len() == 40 && versao.chars().all(|c| c.is_ascii_hexdigit()),
            "a action deve estar fixada por SHA de commit completo: {uso}"
        );
    }
}
