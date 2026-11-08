//! Garante que o workflow de CI mantém as propriedades exigidas pela etapa E02:
//! permissões mínimas, sem segredos, actions fixadas e o mesmo conjunto do `pnpm check`.

use serde_json::Value;

const WORKFLOW: &str = include_str!("../../.github/workflows/ci.yml");
const TEMPLATE: &str = include_str!("../../.github/pull_request_template.md");
const PACKAGE_JSON: &str = include_str!("../../package.json");

/// Linhas do workflow sem comentários e sem linhas vazias.
fn linhas_do_workflow() -> Vec<String> {
    WORKFLOW
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

fn valor_apos(linha: &str, chave: &str) -> Option<String> {
    let linha = linha.trim_start().trim_start_matches("- ");
    linha
        .strip_prefix(chave)
        .map(|resto| resto.trim().to_owned())
}

#[test]
fn roda_em_pull_request_e_em_push_na_branch_principal() {
    let linhas = linhas_do_workflow();

    assert!(linhas.iter().any(|l| l == "  pull_request:"));
    assert!(linhas.iter().any(|l| l == "  push:"));
    assert!(linhas.iter().any(|l| l.trim() == "branches: [main]"));
}

#[test]
fn nao_usa_gatilhos_que_expoem_segredos_a_forks() {
    let texto = linhas_do_workflow().join("\n");

    assert!(!texto.contains("pull_request_target"));
    assert!(!texto.contains("workflow_run"));
}

#[test]
fn declara_apenas_permissao_de_leitura() {
    let linhas = linhas_do_workflow();

    let declaracoes = linhas
        .iter()
        .filter(|l| l.trim_start().starts_with("permissions:"))
        .count();
    assert_eq!(
        declaracoes, 1,
        "deve haver uma única declaração de permissões"
    );

    let posicao = linhas
        .iter()
        .position(|l| l == "permissions:")
        .expect("o workflow deve declarar `permissions:` no nível superior");
    assert_eq!(linhas[posicao + 1], "  contents: read");

    let texto = linhas.join("\n");
    assert!(!texto.contains(": write"));
    assert!(!texto.contains("write-all"));
}

#[test]
fn nao_depende_de_segredos() {
    let texto = linhas_do_workflow().join("\n");

    assert!(!texto.contains("secrets."));
    assert!(!texto.contains("secrets:"));
    assert!(!texto.contains("secrets["));
}

#[test]
fn todas_as_actions_estao_fixadas_por_commit() {
    let usos: Vec<String> = linhas_do_workflow()
        .iter()
        .filter_map(|l| valor_apos(l, "uses:"))
        .collect();

    assert!(!usos.is_empty(), "o workflow deve usar ao menos uma action");
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

#[test]
fn roda_em_runner_windows() {
    let linhas = linhas_do_workflow();

    assert!(
        linhas
            .iter()
            .filter_map(|l| valor_apos(l, "runs-on:"))
            .all(|runner| runner.starts_with("windows-")),
        "todos os jobs devem rodar em runner Windows"
    );
    assert!(
        linhas
            .iter()
            .any(|l| l.trim_start().starts_with("runs-on:"))
    );
}

#[test]
fn roda_o_mesmo_comando_do_ambiente_local_sem_verificacoes_extras() {
    let comandos: Vec<String> = linhas_do_workflow()
        .iter()
        .filter_map(|l| valor_apos(l, "run:"))
        .collect();

    assert!(comandos.iter().any(|c| c == "pnpm check"));
    for comando in comandos {
        let permitido = comando == "pnpm check"
            || comando == "pnpm install --frozen-lockfile"
            || comando.starts_with("rustup toolchain install");
        assert!(
            permitido,
            "o CI só pode preparar o ambiente e rodar `pnpm check`: {comando}"
        );
    }
}

#[test]
fn comando_local_cobre_todas_as_verificacoes() {
    let pacote: Value = serde_json::from_str(PACKAGE_JSON).expect("package.json válido");
    let check = pacote["scripts"]["check"]
        .as_str()
        .expect("o script `check` deve existir");

    for etapa in [
        "pnpm format:check",
        "pnpm lint",
        "pnpm typecheck",
        "pnpm test",
        "pnpm build",
        "pnpm rust:fmt",
        "pnpm rust:lint",
        "pnpm rust:test",
    ] {
        assert!(check.contains(etapa), "`pnpm check` deve rodar `{etapa}`");
    }
}

#[test]
fn template_de_pr_tem_o_checklist_da_etapa() {
    let template = TEMPLATE.to_lowercase();

    for item in [
        "etapa",
        "critérios de aceite",
        "teste",
        "readme.md",
        "registro de decisões",
    ] {
        assert!(template.contains(item), "o template deve citar: {item}");
    }
    assert!(
        TEMPLATE.contains("- [ ]"),
        "o template deve ter caixas de seleção"
    );
}
