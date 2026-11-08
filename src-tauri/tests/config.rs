use serde_json::Value;

fn config() -> Value {
    serde_json::from_str(include_str!("../tauri.conf.json"))
        .expect("tauri.conf.json deve ser um JSON válido")
}

#[test]
fn nome_do_app_e_memoteca() {
    let config = config();

    assert_eq!(config["productName"], "Memoteca");
    assert_eq!(config["app"]["windows"][0]["title"], "Memoteca");
}

#[test]
fn identificador_nao_usa_a_marca_ollama() {
    let config = config();
    let identificador = config["identifier"]
        .as_str()
        .expect("identifier deve ser texto")
        .to_lowercase();

    assert!(!identificador.contains("ollama"));
}

#[test]
fn manifesto_do_pacote_usa_o_nome_do_projeto() {
    let manifesto = include_str!("../Cargo.toml").to_lowercase();

    assert!(!manifesto.contains("ollamaui"));
}
