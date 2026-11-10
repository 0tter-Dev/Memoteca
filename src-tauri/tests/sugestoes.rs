//! Testes de aceite da E12: sugestão de memórias sob demanda, contra o servidor Ollama falso.
//! Os critérios de falha do plano levam "(falha)" no nome, e o cenário do plano de
//! robustez leva o identificador `Rnn`.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::sync::Mutex;
use std::time::Duration;

use memoteca_lib::banco::{Banco, DadosConversa, DadosProjeto, Mensagem, Papel};
use memoteca_lib::conversas::sugestoes::MAXIMO_DE_SUGESTOES;
use memoteca_lib::conversas::{
    DesfechoDaSugestao, ErroSugestao, SugestaoDeMemoria, registrar_mensagem_do_usuario, sugerir,
};
use memoteca_lib::ollama::{ClienteOllama, ErroOllama, Limites};
use ollama_falso::{OllamaFalso, Resposta, endereco_sem_ninguem};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

// ----- Apoio -----

fn limites(ociosidade: Duration) -> Limites {
    Limites {
        conexao: Duration::from_secs(4),
        consulta: Duration::from_secs(6),
        ociosidade_chat: ociosidade,
        ociosidade_pull: Duration::from_millis(800),
    }
}

/// Resposta do `/api/chat` sem streaming, com este texto em `message.content`.
fn resposta_com(conteudo: &str) -> Resposta {
    Resposta::json(
        &json!({
            "model": "m",
            "message": { "role": "assistant", "content": conteudo },
            "done": true,
            "done_reason": "stop"
        })
        .to_string(),
    )
}

fn sugestoes_em_json(textos: &[&str]) -> String {
    let itens: Vec<Value> = textos.iter().map(|t| json!({ "texto": t })).collect();
    json!({ "sugestoes": itens }).to_string()
}

async fn servidor(chat: Resposta) -> OllamaFalso {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion","thinking"],"model_info":{"qwen3.context_length":40960}}"#,
        ),
    );
    servidor.rota("POST", "/api/chat", chat);
    servidor
}

struct Cenario {
    banco: Mutex<Banco>,
    cliente: ClienteOllama,
    projeto_id: String,
    conversa_id: String,
}

/// Um projeto com uma conversa que já tem uma troca de mensagens (modelo `qwen3:8b`).
fn cenario(url: &str, ociosidade: Duration) -> Cenario {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "Projeto".into(),
            system_prompt: "Instruções do projeto.".into(),
            modelo_padrao: Some("qwen3:8b".into()),
        })
        .unwrap();
    let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
    let banco = Mutex::new(banco);
    registrar_mensagem_do_usuario(&banco, &conversa.id, "Sou a Marina e trabalho com Rust.")
        .unwrap();
    banco
        .lock()
        .unwrap()
        .adicionar_mensagem(&conversa.id, Papel::Assistente, "Olá, Marina!", None)
        .unwrap();
    Cenario {
        banco,
        cliente: ClienteOllama::com_endereco(url, limites(ociosidade)).unwrap(),
        projeto_id: projeto.id,
        conversa_id: conversa.id,
    }
}

impl Cenario {
    fn mensagens(&self) -> Vec<Mensagem> {
        self.banco
            .lock()
            .unwrap()
            .listar_mensagens(&self.conversa_id)
            .unwrap()
    }

    /// Quantas memórias existem no total (globais mais as do projeto).
    fn memorias(&self) -> usize {
        let banco = self.banco.lock().unwrap();
        banco.listar_memorias_globais().unwrap().len()
            + banco
                .listar_memorias_do_projeto(&self.projeto_id)
                .unwrap()
                .len()
    }

    async fn sugerir(&self) -> Result<DesfechoDaSugestao, ErroSugestao> {
        sugerir(
            &self.banco,
            &self.cliente,
            &self.conversa_id,
            &CancellationToken::new(),
        )
        .await
    }
}

fn corpos_do_chat(servidor: &OllamaFalso) -> Vec<Value> {
    servidor
        .requisicoes()
        .into_iter()
        .filter(|r| r.caminho == "/api/chat")
        .map(|r| serde_json::from_str(&r.corpo).unwrap())
        .collect()
}

fn textos(desfecho: DesfechoDaSugestao) -> Vec<String> {
    match desfecho {
        DesfechoDaSugestao::Sugestoes(lista) => lista
            .into_iter()
            .map(|SugestaoDeMemoria { texto }| texto)
            .collect(),
        DesfechoDaSugestao::Cancelada => panic!("não deveria ter sido cancelada"),
    }
}

const LONGO: Duration = Duration::from_secs(6);

// ----- O pedido -----

#[tokio::test]
async fn o_pedido_e_sem_stream_com_esquema_sem_raciocinio_e_usa_o_modelo_da_conversa() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&[
        "O usuário se chama Marina.",
    ])))
    .await;
    let c = cenario(&servidor.url(), LONGO);

    let sugestoes = textos(c.sugerir().await.unwrap());

    assert_eq!(sugestoes, ["O usuário se chama Marina."]);
    let corpos = corpos_do_chat(&servidor);
    assert_eq!(corpos.len(), 1, "uma única chamada extra");
    let corpo = &corpos[0];
    assert_eq!(corpo["model"], "qwen3:8b");
    assert_eq!(corpo["stream"], false);
    assert_eq!(corpo["think"], false);
    assert_eq!(corpo["format"]["required"], json!(["sugestoes"]));
    assert_eq!(corpo["options"]["num_ctx"], 8192);
    assert_eq!(corpo["keep_alive"], "5m");
    // A instrução vai como system, e a conversa (com as duas falas) vai no pedido.
    let mensagens = corpo["messages"].as_array().unwrap();
    assert_eq!(mensagens[0]["role"], "system");
    let pedido = mensagens[1]["content"].as_str().unwrap();
    assert!(pedido.contains("Usuário: Sou a Marina e trabalho com Rust."));
    assert!(pedido.contains("Assistente: Olá, Marina!"));
    // As instruções do projeto não são memória e não entram na extração.
    assert!(!corpo.to_string().contains("Instruções do projeto."));
}

#[tokio::test]
async fn as_memorias_que_ja_existem_vao_no_pedido_e_nao_voltam_como_sugestao() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&[
        "Mora em Recife",
        "O usuário trabalha com Rust.",
    ])))
    .await;
    let c = cenario(&servidor.url(), LONGO);
    c.banco
        .lock()
        .unwrap()
        .criar_memoria_global("Mora em Recife")
        .unwrap();
    c.banco
        .lock()
        .unwrap()
        .criar_memoria_de_projeto(&c.projeto_id, "Prefere respostas curtas")
        .unwrap();

    let sugestoes = textos(c.sugerir().await.unwrap());

    assert_eq!(sugestoes, ["O usuário trabalha com Rust."]);
    let sistema = corpos_do_chat(&servidor)[0]["messages"][0]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(sistema.contains("- Mora em Recife"));
    assert!(sistema.contains("- Prefere respostas curtas"));
}

#[tokio::test]
async fn o_num_ctx_da_conversa_e_limitado_a_janela_do_modelo() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion"],"model_info":{"llama.context_length":4096}}"#,
        ),
    );
    servidor.rota("POST", "/api/chat", resposta_com(&sugestoes_em_json(&[])));
    let c = cenario(&servidor.url(), LONGO);

    c.sugerir().await.unwrap();

    assert_eq!(corpos_do_chat(&servidor)[0]["options"]["num_ctx"], 4096);
}

#[tokio::test]
async fn nunca_vem_mais_que_o_maximo_de_sugestoes() {
    let muitas: Vec<String> = (0..9).map(|i| format!("Fato {i}")).collect();
    let referencias: Vec<&str> = muitas.iter().map(String::as_str).collect();
    let servidor = servidor(resposta_com(&sugestoes_em_json(&referencias))).await;
    let c = cenario(&servidor.url(), LONGO);

    assert_eq!(
        textos(c.sugerir().await.unwrap()).len(),
        MAXIMO_DE_SUGESTOES
    );
}

// ----- Nada é criado sem aprovação; o histórico não muda -----

#[tokio::test]
async fn pedir_sugestoes_nao_cria_memoria_nem_altera_o_historico() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&[
        "O usuário se chama Marina.",
        "O usuário trabalha com Rust.",
    ])))
    .await;
    let c = cenario(&servidor.url(), LONGO);
    let antes = c.mensagens();
    let conversa_antes = c
        .banco
        .lock()
        .unwrap()
        .obter_conversa(&c.conversa_id)
        .unwrap();

    let sugestoes = textos(c.sugerir().await.unwrap());

    assert_eq!(sugestoes.len(), 2);
    assert_eq!(c.memorias(), 0, "sugerir não cria memória");
    assert_eq!(c.mensagens(), antes, "o histórico fica exatamente igual");
    let conversa_depois = c
        .banco
        .lock()
        .unwrap()
        .obter_conversa(&c.conversa_id)
        .unwrap();
    assert_eq!(conversa_depois, conversa_antes, "nem a conversa é tocada");
}

#[tokio::test]
async fn rejeitar_ou_ignorar_as_sugestoes_nao_deixa_nenhuma_memoria() {
    // O backend só devolve a lista. Descartá-la (rejeitar, fechar a tela) não passa por ele,
    // então o estado do banco depois de gerar já é o estado final desse caminho.
    let servidor = servidor(resposta_com(&sugestoes_em_json(&["Uma", "Outra"]))).await;
    let c = cenario(&servidor.url(), LONGO);

    let _ = c.sugerir().await.unwrap();

    assert_eq!(c.memorias(), 0);
}

#[tokio::test]
async fn so_a_aprovacao_cria_uma_memoria_pelo_mesmo_caminho_das_manuais() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&["Uma", "Outra"]))).await;
    let c = cenario(&servidor.url(), LONGO);
    let sugestoes = textos(c.sugerir().await.unwrap());

    // A tela aprova a primeira como global e rejeita a segunda.
    c.banco
        .lock()
        .unwrap()
        .criar_memoria_global(&sugestoes[0])
        .unwrap();

    assert_eq!(c.memorias(), 1);
    let globais = c.banco.lock().unwrap().listar_memorias_globais().unwrap();
    assert_eq!(globais[0].conteudo, "Uma");
}

#[tokio::test]
async fn lista_vazia_do_modelo_nao_e_erro_e_nao_cria_nada() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&[]))).await;
    let c = cenario(&servidor.url(), LONGO);

    assert_eq!(textos(c.sugerir().await.unwrap()), Vec::<String>::new());
    assert_eq!(c.memorias(), 0);
}

// ----- R31: resposta malformada ou vazia (falha) -----

#[tokio::test]
async fn r31_resposta_malformada_ou_vazia_da_erro_amigavel_e_nao_cria_memoria() {
    let invalidas = [
        "",
        "   ",
        "isto não é JSON",
        "{}",
        r#"{"sugestoes":"nada"}"#,
        r#"{"sugestoes":[{"texto":7}]}"#,
        r#"{"sugestoes":[{"texto":"cortado"#,
        r#"{"outra":[{"texto":"x"}]}"#,
    ];
    for conteudo in invalidas {
        let servidor = servidor(resposta_com(conteudo)).await;
        let c = cenario(&servidor.url(), LONGO);
        let antes = c.mensagens();

        let erro = c.sugerir().await.unwrap_err();

        assert_eq!(erro, ErroSugestao::RespostaInvalida, "{conteudo:?}");
        let amigavel = erro.mensagem_amigavel();
        assert!(
            amigavel.contains("Nenhuma memória foi criada"),
            "{amigavel}"
        );
        assert!(
            !amigavel.contains("JSON"),
            "nada técnico para o usuário: {amigavel}"
        );
        assert_eq!(c.memorias(), 0, "{conteudo:?}");
        assert_eq!(c.mensagens(), antes, "{conteudo:?}");
    }
}

#[tokio::test]
async fn r31_corpo_http_que_nao_e_json_tambem_e_tratado() {
    let servidor = servidor(Resposta::json("<html>não sou o Ollama</html>")).await;
    let c = cenario(&servidor.url(), LONGO);

    let erro = c.sugerir().await.unwrap_err();

    assert!(matches!(
        erro,
        ErroSugestao::Ollama(ErroOllama::RespostaInvalida(_))
    ));
    assert!(!erro.mensagem_amigavel().contains("html"));
    assert_eq!(c.memorias(), 0);
}

// ----- Outras falhas -----

#[tokio::test]
async fn ollama_fora_do_ar_da_erro_amigavel_e_nada_muda() {
    let url = endereco_sem_ninguem().await;
    let c = cenario(&url, LONGO);
    let antes = c.mensagens();

    let erro = c.sugerir().await.unwrap_err();

    assert_eq!(erro, ErroSugestao::Ollama(ErroOllama::OllamaForaDoAr));
    assert!(erro.mensagem_amigavel().contains("Ollama"));
    assert_eq!(c.memorias(), 0);
    assert_eq!(c.mensagens(), antes);
}

#[tokio::test]
async fn erro_do_ollama_vira_mensagem_amigavel_sem_o_texto_cru() {
    let servidor = servidor(Resposta::erro(500, "falha interna do runner")).await;
    let c = cenario(&servidor.url(), LONGO);

    let erro = c.sugerir().await.unwrap_err();

    assert!(matches!(
        erro,
        ErroSugestao::Ollama(ErroOllama::ErroDoOllama(_))
    ));
    assert!(!erro.mensagem_amigavel().contains("runner"));
    assert_eq!(c.memorias(), 0);
}

#[tokio::test]
async fn modelo_removido_e_modelo_sem_conversa_dao_erro_amigavel() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::erro(404, "model 'qwen3:8b' not found"),
    );
    let c = cenario(&servidor.url(), LONGO);
    assert_eq!(
        c.sugerir().await.unwrap_err(),
        ErroSugestao::Ollama(ErroOllama::ModeloNaoEncontrado)
    );

    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(r#"{"capabilities":["embedding"],"model_info":{}}"#),
    );
    let c = cenario(&servidor.url(), LONGO);
    assert_eq!(
        c.sugerir().await.unwrap_err(),
        ErroSugestao::Ollama(ErroOllama::ModeloSemSuporte)
    );
    assert_eq!(c.memorias(), 0);
}

#[tokio::test]
async fn ollama_que_nao_responde_da_tempo_esgotado() {
    let servidor = servidor(Resposta::NaoResponder).await;
    let c = cenario(&servidor.url(), Duration::from_millis(400));

    let erro = c.sugerir().await.unwrap_err();

    assert_eq!(erro, ErroSugestao::Ollama(ErroOllama::TempoEsgotado));
    assert_eq!(c.memorias(), 0);
}

#[tokio::test]
async fn conversa_sem_mensagens_e_conversa_sem_modelo_dao_erro_amigavel_sem_chamar_o_modelo() {
    let servidor = servidor(resposta_com(&sugestoes_em_json(&["x"]))).await;
    let c = cenario(&servidor.url(), LONGO);

    // Conversa nova, sem nenhuma mensagem.
    let vazia = c
        .banco
        .lock()
        .unwrap()
        .criar_conversa_nova(&c.projeto_id)
        .unwrap();
    let erro = memoteca_lib::conversas::sugerir(
        &c.banco,
        &c.cliente,
        &vazia.id,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(erro, ErroSugestao::ConversaVazia);

    // Conversa sem modelo.
    {
        let banco = c.banco.lock().unwrap();
        let atual = banco.obter_conversa(&c.conversa_id).unwrap();
        banco
            .atualizar_conversa(
                &c.conversa_id,
                &DadosConversa {
                    titulo: atual.titulo,
                    modelo: None,
                    temperature: atual.temperature,
                    num_ctx: atual.num_ctx,
                    keep_alive: atual.keep_alive,
                },
            )
            .unwrap();
    }
    assert_eq!(c.sugerir().await.unwrap_err(), ErroSugestao::SemModelo);

    assert!(
        corpos_do_chat(&servidor).is_empty(),
        "o modelo nem foi chamado"
    );
}

// ----- Cancelar -----

#[tokio::test]
async fn cancelar_a_chamada_fecha_a_conexao_e_nao_cria_nada() {
    let servidor = servidor(Resposta::NaoResponder).await;
    let c = cenario(&servidor.url(), LONGO);
    let antes = c.mensagens();
    let cancelamento = CancellationToken::new();
    let gatilho = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        gatilho.cancel();
    });

    let desfecho =
        memoteca_lib::conversas::sugerir(&c.banco, &c.cliente, &c.conversa_id, &cancelamento)
            .await
            .unwrap();

    assert_eq!(desfecho, DesfechoDaSugestao::Cancelada);
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await,
        "cancelar fecha a conexão com o Ollama"
    );
    assert_eq!(c.memorias(), 0);
    assert_eq!(c.mensagens(), antes);
}
