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
fn roda_o_comando_local_e_depois_os_testes_ponta_a_ponta_sem_verificacoes_extras() {
    let comandos: Vec<String> = linhas_do_workflow()
        .iter()
        .filter_map(|l| valor_apos(l, "run:"))
        .collect();

    assert!(comandos.iter().any(|c| c == "pnpm check"));
    for comando in comandos {
        let permitido = comando == "pnpm check"
            || comando == "pnpm e2e"
            || comando == "pnpm install --frozen-lockfile"
            || comando.starts_with("rustup toolchain install");
        assert!(
            permitido,
            "o CI só pode preparar o ambiente e rodar `pnpm check` e `pnpm e2e`: {comando}"
        );
    }
}

/// A E15 exige que os testes ponta a ponta rodem no workflow, e que uma regressão derrube o
/// check. O passo `pnpm e2e` vem depois do `pnpm check`, no mesmo job (o check obrigatório), e
/// não pode ser tolerado a falhar.
#[test]
fn os_testes_ponta_a_ponta_rodam_depois_do_check_e_nao_podem_falhar_em_silencio() {
    let linhas = linhas_do_workflow();
    let posicao_do_check = linhas
        .iter()
        .position(|l| valor_apos(l, "run:").as_deref() == Some("pnpm check"))
        .expect("o CI deve rodar `pnpm check`");
    let posicao_do_e2e = linhas
        .iter()
        .position(|l| valor_apos(l, "run:").as_deref() == Some("pnpm e2e"))
        .expect("o CI deve rodar `pnpm e2e`");

    assert!(posicao_do_check < posicao_do_e2e);
    assert!(
        !linhas.iter().any(|l| l.contains("continue-on-error")),
        "nenhum passo pode ter `continue-on-error`, senão a regressão não derruba o check"
    );
}

#[test]
fn o_script_e2e_existe_e_roda_o_playwright_sobre_o_build_de_testes() {
    let pacote: Value = serde_json::from_str(PACKAGE_JSON).expect("package.json válido");
    let e2e = pacote["scripts"]["e2e"].as_str().expect("script `e2e`");
    let construir = pacote["scripts"]["e2e:build"]
        .as_str()
        .expect("script `e2e:build`");

    assert!(e2e.contains("pnpm e2e:build") && e2e.contains("playwright test"));
    assert!(construir.contains("--features e2e"));
    assert!(construir.contains("ollama_falso"));
    // O E2E não entra no `pnpm check`: ele se recusa a rodar onde há um Ollama de verdade.
    let check = pacote["scripts"]["check"].as_str().unwrap();
    assert!(!check.contains("e2e"));
}

/// O recurso `e2e` troca a pasta de dados, as janelas de arquivo, o instalador e o espaço em
/// disco por versões de teste. Nunca pode estar ativo num build normal nem no de release.
#[test]
fn o_recurso_e2e_nunca_e_ativado_por_padrao_nem_no_build_de_release() {
    let cargo = include_str!("../Cargo.toml");
    let tauri = include_str!("../tauri.conf.json");
    let pacote: Value = serde_json::from_str(PACKAGE_JSON).expect("package.json válido");

    assert!(cargo.contains("e2e = []"), "o recurso `e2e` deve existir");
    assert!(
        !cargo.lines().any(|l| l.trim_start().starts_with("default")),
        "nenhum recurso pode ser ativado por padrão"
    );
    assert!(
        !tauri.contains("e2e"),
        "o tauri.conf.json não pode ligar o recurso"
    );

    // Só o script `e2e:build` liga o recurso. O `build` e o `tauri` não.
    let scripts = pacote["scripts"].as_object().expect("scripts");
    for (nome, comando) in scripts {
        let comando = comando.as_str().unwrap_or_default();
        if comando.contains("--features e2e") {
            assert_eq!(
                nome, "e2e:build",
                "só `e2e:build` pode ligar o recurso `e2e`"
            );
        }
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
