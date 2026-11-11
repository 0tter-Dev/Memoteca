//! Servidor Ollama falso executável, para os testes ponta a ponta (E15).
//!
//! É o mesmo servidor falso dos testes de integração (`tests/support/ollama_falso.rs`), com
//! as respostas de um Ollama que funciona e alguns cenários de falha que o teste escolhe
//! durante a execução. Nada daqui entra no app.
//!
//! Uso:
//!
//! - `ollama_falso [--porta N]`: sobe o servidor (padrão `127.0.0.1:11434`).
//! - Chamado como `ollama.exe serve`, depois de o instalador falso do app copiá-lo para a
//!   pasta do Ollama, ele se comporta como o Ollama que o app iniciou.
//!
//! Variáveis de ambiente: `FALSO_MODELOS` (modelos já instalados, separados por vírgula).
//!
//! Rotas de controle do teste (não existem no Ollama de verdade):
//!
//! - `POST /__e2e/cenario` com `{"cenario": "<nome>"}`: escolhe como o servidor responde.
//! - `GET /__e2e/requisicoes`: todos os pedidos recebidos, em JSON.
//!
//! Cenários: `normal`; `chat_corta` (a conexão cai no meio da resposta); `pull_interrompido`
//! (o próximo download cai no meio, uma vez só, e o seguinte funciona).

#[path = "../tests/support/ollama_falso.rs"]
mod ollama_falso;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ollama_falso::{OllamaFalso, Passo, Requisicao, Resposta};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const VERSAO: &str = "0.40.15";

/// Conteúdo do "instalador" servido em `/OllamaSetup.exe`. O app de teste não o executa.
const INSTALADOR_FALSO: &[u8] = b"instalador falso do E2E: nao e um programa";

struct Estado {
    cenario: String,
    instalados: Vec<String>,
}

type Compartilhado = Arc<Mutex<Estado>>;

fn porta_pedida() -> u16 {
    let mut argumentos = std::env::args().skip(1);
    while let Some(argumento) = argumentos.next() {
        if argumento == "--porta"
            && let Some(valor) = argumentos.next().and_then(|v| v.parse().ok())
        {
            return valor;
        }
    }
    11434
}

fn linha_conteudo(texto: &str) -> String {
    json!({"model": "falso", "message": {"role": "assistant", "content": texto}, "done": false})
        .to_string()
}

fn linha_final() -> String {
    json!({
        "model": "falso",
        "message": {"role": "assistant", "content": ""},
        "done": true,
        "done_reason": "stop",
        "eval_count": 12,
        "prompt_eval_count": 8
    })
    .to_string()
}

fn linha_pull(total: u64, baixado: u64) -> String {
    json!({"status": "pulling aaa", "digest": "aaa", "total": total, "completed": baixado})
        .to_string()
}

fn tags(estado: &Estado) -> Resposta {
    let modelos: Vec<Value> = estado
        .instalados
        .iter()
        .map(|nome| json!({"name": nome, "size": 1_000_000}))
        .collect();
    Resposta::json(&json!({"models": modelos}).to_string())
}

fn pull(estado: &mut Estado, pedido: &Requisicao) -> Resposta {
    let corpo: Value = serde_json::from_str(&pedido.corpo).unwrap_or(Value::Null);
    let nome = corpo["model"].as_str().unwrap_or("modelo").to_owned();
    let pausa = Passo::Pausa(Duration::from_millis(120));

    if estado.cenario == "pull_interrompido" {
        // Cai uma vez só: o "Continuar" do usuário baixa o resto.
        estado.cenario = "normal".to_owned();
        return Resposta::Stream(vec![
            Passo::Linha(r#"{"status":"pulling manifest"}"#.to_owned()),
            Passo::Linha(linha_pull(1000, 0)),
            pausa.clone(),
            Passo::Linha(linha_pull(1000, 400)),
            pausa,
            Passo::Cortar,
        ]);
    }

    if !estado.instalados.contains(&nome) {
        estado.instalados.push(nome);
    }
    Resposta::Stream(vec![
        Passo::Linha(r#"{"status":"pulling manifest"}"#.to_owned()),
        Passo::Linha(linha_pull(1000, 0)),
        pausa.clone(),
        Passo::Linha(linha_pull(1000, 500)),
        pausa.clone(),
        Passo::Linha(linha_pull(1000, 1000)),
        Passo::Linha(r#"{"status":"verifying sha256 digest"}"#.to_owned()),
        Passo::Linha(r#"{"status":"writing manifest"}"#.to_owned()),
        Passo::Linha(r#"{"status":"success"}"#.to_owned()),
        Passo::Terminar,
    ])
}

fn chat(estado: &Estado, pedido: &Requisicao) -> Resposta {
    let corpo: Value = serde_json::from_str(&pedido.corpo).unwrap_or(Value::Null);

    // Sugestões de memória: sem streaming e com a resposta presa a um esquema.
    if corpo["stream"] == Value::Bool(false) {
        let sugestoes = json!({"sugestoes": [
            {"texto": "O usuário trabalha com Rust."},
            {"texto": "O usuário prefere respostas curtas."}
        ]});
        return Resposta::json(
            &json!({
                "model": "falso",
                "message": {"role": "assistant", "content": sugestoes.to_string()},
                "done": true
            })
            .to_string(),
        );
    }

    let pausa = Passo::Pausa(Duration::from_millis(60));
    let partes = ["Olá! ", "Esta é ", "uma resposta ", "de teste."];
    let mut passos = Vec::new();
    if estado.cenario == "chat_corta" {
        passos.push(Passo::Linha(linha_conteudo(partes[0])));
        passos.push(pausa);
        passos.push(Passo::Linha(linha_conteudo(partes[1])));
        passos.push(Passo::Pausa(Duration::from_millis(100)));
        passos.push(Passo::Cortar);
        return Resposta::Stream(passos);
    }
    for parte in partes {
        passos.push(Passo::Linha(linha_conteudo(parte)));
        passos.push(pausa.clone());
    }
    passos.push(Passo::Linha(linha_final()));
    passos.push(Passo::Terminar);
    Resposta::Stream(passos)
}

#[tokio::main]
async fn main() {
    let porta = porta_pedida();
    let instalados = std::env::var("FALSO_MODELOS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|nome| !nome.is_empty())
        .map(str::to_owned)
        .collect();
    let estado: Compartilhado = Arc::new(Mutex::new(Estado {
        cenario: "normal".to_owned(),
        instalados,
    }));

    let servidor = OllamaFalso::iniciar_em(&format!("127.0.0.1:{porta}")).await;

    servidor.rota(
        "GET",
        "/api/version",
        Resposta::json(&json!({"version": VERSAO}).to_string()),
    );
    let e = Arc::clone(&estado);
    servidor.rota(
        "GET",
        "/api/tags",
        Resposta::Dinamica(Arc::new(move |_, _| tags(&e.lock().unwrap()))),
    );
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion"],"model_info":{"falso.context_length":8192}}"#,
        ),
    );
    let e = Arc::clone(&estado);
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Dinamica(Arc::new(move |pedido, _| {
            pull(&mut e.lock().unwrap(), pedido)
        })),
    );
    let e = Arc::clone(&estado);
    servidor.rota(
        "POST",
        "/api/chat",
        Resposta::Dinamica(Arc::new(move |pedido, _| chat(&e.lock().unwrap(), pedido))),
    );

    // O que o instalador falso do app baixa: a "release" e o "instalador".
    let sha256: String = Sha256::digest(INSTALADOR_FALSO)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let release = json!({
        "tag_name": format!("v{VERSAO}"),
        "assets": [{
            "name": "OllamaSetup.exe",
            "size": INSTALADOR_FALSO.len(),
            "digest": format!("sha256:{sha256}"),
            "browser_download_url": format!("http://127.0.0.1:{porta}/OllamaSetup.exe"),
        }]
    });
    servidor.rota("GET", "/release", Resposta::json(&release.to_string()));
    servidor.rota(
        "GET",
        "/OllamaSetup.exe",
        Resposta::Binario(INSTALADOR_FALSO.to_vec()),
    );

    // Controle do teste.
    let e = Arc::clone(&estado);
    servidor.rota(
        "POST",
        "/__e2e/cenario",
        Resposta::Dinamica(Arc::new(move |pedido, _| {
            let corpo: Value = serde_json::from_str(&pedido.corpo).unwrap_or(Value::Null);
            if let Some(nome) = corpo["cenario"].as_str() {
                e.lock().unwrap().cenario = nome.to_owned();
            }
            Resposta::json("{}")
        })),
    );
    servidor.rota(
        "GET",
        "/__e2e/requisicoes",
        Resposta::Dinamica(Arc::new(|_, historico| {
            let itens: Vec<Value> = historico
                .iter()
                .filter(|r| !r.caminho.starts_with("/__e2e/"))
                .map(|r| json!({"metodo": r.metodo, "caminho": r.caminho, "corpo": r.corpo}))
                .collect();
            Resposta::json(&Value::Array(itens).to_string())
        })),
    );

    // Fica no ar até o processo ser encerrado (pelo app, pelo Job Object ou pelo teste).
    std::future::pending::<()>().await;
}
