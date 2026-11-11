//! Conferências da E14 sobre o plano de robustez e as mensagens de erro.
//!
//! * Todo cenário `Rnn` do `PLANO.md` tem ao menos um teste que o cita (no nome do teste Rust,
//!   `rNN_...`, ou no nome de um teste do frontend).
//! * As mensagens escritas direto nos comandos do app (`lib.rs`) seguem as mesmas regras do
//!   catálogo (`memoteca_lib::catalogo`), que já cobre as mensagens dos tipos de erro.

use std::fs;
use std::path::{Path, PathBuf};

use memoteca_lib::catalogo::{Entrada, problemas};

const RAIZ: &str = env!("CARGO_MANIFEST_DIR");

fn arquivos(pasta: &Path, extensoes: &[&str], achados: &mut Vec<PathBuf>) {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let nome = entrada.file_name().to_string_lossy().into_owned();
        if caminho.is_dir() {
            if !matches!(nome.as_str(), "target" | "node_modules" | ".git" | "gen") {
                arquivos(&caminho, extensoes, achados);
            }
        } else if extensoes.iter().any(|e| nome.ends_with(e)) {
            achados.push(caminho);
        }
    }
}

fn ler(caminho: &Path) -> String {
    fs::read_to_string(caminho).unwrap_or_default()
}

/// Um cenário da tabela do plano de robustez.
struct Cenario {
    id: String,
    etapas: Vec<u32>,
}

fn cenarios_do_plano() -> Vec<Cenario> {
    let plano = ler(&Path::new(RAIZ).join("../PLANO.md"));
    assert!(
        plano.contains("Plano de testes de robustez"),
        "PLANO.md não encontrado"
    );
    plano
        .lines()
        .filter_map(|linha| {
            let colunas: Vec<&str> = linha.split('|').map(str::trim).collect();
            // `| R01 | cenário | como | esperado | etapas |` gera 7 colunas, com as pontas vazias.
            let id = colunas.get(1)?;
            let bytes = id.as_bytes();
            let eh_cenario = bytes.len() == 3
                && bytes[0] == b'R'
                && bytes[1].is_ascii_digit()
                && bytes[2].is_ascii_digit();
            if !eh_cenario || colunas.len() < 7 {
                return None;
            }
            let etapas = colunas[colunas.len() - 2]
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter_map(|trecho| trecho.strip_prefix('E')?.parse().ok())
                .collect();
            Some(Cenario {
                id: (*id).to_owned(),
                etapas,
            })
        })
        .collect()
}

#[test]
fn o_plano_de_robustez_foi_lido_inteiro() {
    let cenarios = cenarios_do_plano();

    assert_eq!(cenarios.len(), 33, "R01 a R33");
    assert_eq!(cenarios[0].id, "R01");
    assert_eq!(cenarios[32].id, "R33");
    assert!(cenarios.iter().all(|c| !c.etapas.is_empty()));
}

#[test]
fn todo_cenario_do_plano_de_robustez_tem_ao_menos_um_teste() {
    let mut rust = Vec::new();
    arquivos(&Path::new(RAIZ).join("tests"), &[".rs"], &mut rust);
    arquivos(&Path::new(RAIZ).join("src"), &[".rs"], &mut rust);
    let mut web = Vec::new();
    arquivos(
        &Path::new(RAIZ).join("../src"),
        &[".test.tsx", ".test.ts"],
        &mut web,
    );
    let rust: Vec<String> = rust.iter().map(|c| ler(c)).collect();
    let web: Vec<String> = web.iter().map(|c| ler(c)).collect();

    let mut sem_teste = Vec::new();
    for cenario in cenarios_do_plano() {
        // Um cenário cujo teste só é previsto para a E15 (o ponta a ponta) fica de fora.
        if cenario.etapas.iter().all(|etapa| *etapa >= 15) {
            continue;
        }
        let minusculo = cenario.id.to_lowercase();
        let em_rust = rust
            .iter()
            .any(|fonte| fonte.contains(&format!("fn {minusculo}_")));
        let no_frontend = web.iter().any(|fonte| {
            fonte.contains(&format!("{} ", cenario.id))
                || fonte.contains(&format!("{}:", cenario.id))
        });
        if !em_rust && !no_frontend {
            sem_teste.push(cenario.id);
        }
    }

    assert!(
        sem_teste.is_empty(),
        "cenários do plano de robustez sem nenhum teste: {sem_teste:?}"
    );
}

/// Os literais de texto do arquivo (sem os comentários), com o que está entre aspas.
fn literais(fonte: &str) -> Vec<String> {
    let mut achados = Vec::new();
    for linha in fonte.lines() {
        let aparada = linha.trim_start();
        if aparada.starts_with("//") {
            continue;
        }
        let mut dentro = false;
        let mut atual = String::new();
        let mut escapado = false;
        for c in linha.chars() {
            if dentro {
                if escapado {
                    atual.push(c);
                    escapado = false;
                } else if c == '\\' {
                    escapado = true;
                } else if c == '"' {
                    achados.push(std::mem::take(&mut atual));
                    dentro = false;
                } else {
                    atual.push(c);
                }
            } else if c == '"' {
                dentro = true;
            }
        }
    }
    achados
}

#[test]
fn as_mensagens_escritas_direto_nos_comandos_seguem_as_regras_do_catalogo() {
    let fonte = ler(&Path::new(RAIZ).join("src/lib.rs"));
    // Só os testes ficam de fora: o resto do arquivo é código de produção.
    let producao = fonte.split("#[cfg(test)]").next().unwrap();

    let mut recusadas = Vec::new();
    let mut conferidas = 0;
    for texto in literais(producao) {
        // Uma mensagem para o usuário é uma frase em português: começa com maiúscula, tem
        // espaços e termina com ponto. Logs começam com minúscula, e o resto não é frase.
        let parece_mensagem = texto.chars().next().is_some_and(char::is_uppercase)
            && texto.contains(' ')
            && texto.ends_with('.')
            && !texto.contains('{');
        if !parece_mensagem {
            continue;
        }
        conferidas += 1;
        let entrada = Entrada {
            origem: "lib.rs".into(),
            texto: texto.clone(),
            informativa: false,
        };
        let achados = problemas(&entrada);
        if !achados.is_empty() {
            recusadas.push(format!("{texto:?} -> {}", achados.join("; ")));
        }
    }

    assert!(conferidas >= 5, "achou só {conferidas} mensagens em lib.rs");
    assert!(
        recusadas.is_empty(),
        "mensagens fora das regras:\n{}",
        recusadas.join("\n")
    );
}
