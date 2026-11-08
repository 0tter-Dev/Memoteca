//! Testes de aceite da E05: cliente da API do Ollama contra o servidor falso.
//! Nenhum teste fala com um Ollama real. Os testes dos cenários do plano de robustez citam
//! o identificador `Rnn` no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::time::Duration;

use memoteca_lib::ollama::{
    ClienteOllama, ErroOllama, FalhaChat, KeepAlive, Limites, MensagemPedido, MotivoFim, PapelChat,
    PedacoChat, PedidoChat, ProgressoDownload, RespostaChat, ResultadoDownload, Versao,
};
use ollama_falso::{OllamaFalso, Passo, Resposta, endereco_sem_ninguem};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

// ----- Apoio -----

fn limites_curtos() -> Limites {
    Limites {
        conexao: Duration::from_millis(500),
        consulta: Duration::from_millis(500),
        ociosidade_chat: Duration::from_millis(400),
        ociosidade_pull: Duration::from_millis(400),
    }
}

/// Para portas onde ninguém ouve: no Windows a recusa da conexão leva cerca de 2 s.
fn limites_para_porta_fechada() -> Limites {
    Limites {
        conexao: Duration::from_secs(4),
        consulta: Duration::from_secs(6),
        ociosidade_chat: Duration::from_secs(6),
        ociosidade_pull: Duration::from_secs(6),
    }
}

fn cliente_sem_ninguem_na_porta(url: &str) -> ClienteOllama {
    ClienteOllama::com_endereco(url, limites_para_porta_fechada()).unwrap()
}

fn cliente(url: &str) -> ClienteOllama {
    ClienteOllama::com_endereco(url, limites_curtos()).unwrap()
}

fn pedido() -> PedidoChat {
    PedidoChat {
        modelo: "qwen3:8b".into(),
        mensagens: vec![
            MensagemPedido {
                papel: PapelChat::Sistema,
                conteudo: "Seja breve.".into(),
            },
            MensagemPedido {
                papel: PapelChat::Usuario,
                conteudo: "Oi".into(),
            },
        ],
        temperature: Some(0.5),
        num_ctx: Some(4096),
        keep_alive: Some(KeepAlive::Duracao("10m".into())),
    }
}

fn texto_json(texto: &str) -> String {
    serde_json::to_string(texto).unwrap()
}

fn linha_conteudo(texto: &str) -> String {
    format!(
        r#"{{"model":"qwen3:8b","message":{{"role":"assistant","content":{}}},"done":false}}"#,
        texto_json(texto)
    )
}

fn linha_raciocinio(texto: &str) -> String {
    format!(
        r#"{{"model":"qwen3:8b","message":{{"role":"assistant","thinking":{}}},"done":false}}"#,
        texto_json(texto)
    )
}

const LINHA_FINAL: &str = r#"{"model":"qwen3:8b","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","eval_count":3}"#;

/// Conversa e devolve o resultado junto com os pedaços na ordem em que chegaram.
async fn conversar(
    cliente: &ClienteOllama,
    pedido: &PedidoChat,
) -> (Result<RespostaChat, FalhaChat>, Vec<PedacoChat>) {
    let mut pedacos = Vec::new();
    let resultado = cliente
        .conversar(pedido, &CancellationToken::new(), &mut |p| pedacos.push(p))
        .await;
    (resultado, pedacos)
}

async fn servidor_com_chat(resposta: Resposta) -> OllamaFalso {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota("POST", "/api/chat", resposta);
    servidor
}

// ----- Versão e detecção de Ollama (R02, R05, R06) -----

#[tokio::test]
async fn reconhece_um_ollama_pela_versao() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/api/version",
        Resposta::json(r#"{"version":"0.30.2"}"#),
    );

    let info = cliente(&servidor.url()).verificar().await.unwrap();

    assert_eq!(info.versao, Versao::nova(0, 30, 2));
    assert!(info.atende_o_minimo);
    assert_eq!(info.aviso(), None);
}

#[tokio::test]
async fn r05_versao_abaixo_da_minima_vira_aviso_e_nao_erro() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/api/version",
        Resposta::json(r#"{"version":"0.29.9"}"#),
    );

    let info = cliente(&servidor.url()).verificar().await.unwrap();

    assert!(!info.atende_o_minimo);
    let aviso = info.aviso().expect("deve haver aviso");
    assert!(matches!(aviso, ErroOllama::VersaoAntiga { .. }));
    let mensagem = aviso.mensagem_amigavel();
    assert!(mensagem.contains("0.29.9"));
    assert!(mensagem.contains("0.30.0"));
}

#[tokio::test]
async fn r06_versao_com_sufixo_e_aceita_e_formato_invalido_nao_e_ollama() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/api/version",
        Resposta::json(r#"{"version":"0.31.0-rc1"}"#),
    );
    let info = cliente(&servidor.url()).verificar().await.unwrap();
    assert_eq!(info.versao, Versao::nova(0, 31, 0));

    servidor.rota(
        "GET",
        "/api/version",
        Resposta::json(r#"{"version":"abc"}"#),
    );
    assert_eq!(
        cliente(&servidor.url()).verificar().await.unwrap_err(),
        ErroOllama::PortaOcupada
    );
}

#[tokio::test]
async fn r02_porta_ocupada_por_outro_programa_nao_e_confundida_com_ollama() {
    let respostas = [
        ("HTML", Resposta::json("<html><body>Olá</body></html>")),
        ("JSON sem version", Resposta::json(r#"{"status":"ok"}"#)),
        ("HTTP 500", Resposta::erro(500, "algo")),
        ("conexão aceita e fechada", Resposta::AceitarEFechar),
        (
            "bytes que não são HTTP",
            Resposta::NaoEHttp(b"\x00\x01SSH-2.0-qualquer\r\n".to_vec()),
        ),
        ("silêncio", Resposta::NaoResponder),
    ];

    for (descricao, resposta) in respostas {
        let servidor = OllamaFalso::iniciar().await;
        servidor.rota("GET", "/api/version", resposta);

        let erro = cliente(&servidor.url()).verificar().await.unwrap_err();

        assert_eq!(erro, ErroOllama::PortaOcupada, "{descricao}");
        assert!(
            erro.mensagem_amigavel().contains("Outro programa"),
            "{descricao}"
        );
    }
}

#[tokio::test]
async fn sem_ninguem_na_porta_o_erro_e_ollama_fora_do_ar() {
    let cliente = cliente_sem_ninguem_na_porta(&endereco_sem_ninguem().await);

    assert_eq!(
        cliente.verificar().await.unwrap_err(),
        ErroOllama::OllamaForaDoAr
    );
    assert_eq!(
        cliente.listar_modelos().await.unwrap_err(),
        ErroOllama::OllamaForaDoAr
    );
    assert_eq!(
        cliente.detalhes_do_modelo("x").await.unwrap_err(),
        ErroOllama::OllamaForaDoAr
    );
}

// ----- Modelos instalados e detalhes -----

#[tokio::test]
async fn lista_os_modelos_instalados() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/api/tags",
        Resposta::json(
            r#"{"models":[
                {"name":"llama3.2:latest","model":"llama3.2:latest","size":2019393189,
                 "details":{"family":"llama","parameter_size":"3.2B","quantization_level":"Q4_K_M"}},
                {"name":"sem-detalhes:1b"}
            ]}"#,
        ),
    );

    let modelos = cliente(&servidor.url()).listar_modelos().await.unwrap();

    assert_eq!(modelos.len(), 2);
    assert_eq!(modelos[0].nome, "llama3.2:latest");
    assert_eq!(modelos[0].tamanho_em_bytes, Some(2_019_393_189));
    assert_eq!(modelos[0].familia.as_deref(), Some("llama"));
    assert_eq!(modelos[0].tamanho_dos_parametros.as_deref(), Some("3.2B"));
    assert_eq!(modelos[0].quantizacao.as_deref(), Some("Q4_K_M"));
    assert_eq!(modelos[1].nome, "sem-detalhes:1b");
    assert_eq!(modelos[1].familia, None);
}

#[tokio::test]
async fn ollama_sem_modelos_devolve_lista_vazia() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota("GET", "/api/tags", Resposta::json(r#"{"models":[]}"#));

    assert!(
        cliente(&servidor.url())
            .listar_modelos()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn le_capacidades_e_janela_de_contexto_do_modelo() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion","thinking"],
                "model_info":{"general.architecture":"qwen3","qwen3.context_length":40960}}"#,
        ),
    );

    let detalhes = cliente(&servidor.url())
        .detalhes_do_modelo("qwen3:8b")
        .await
        .unwrap();

    assert_eq!(detalhes.janela_de_contexto, Some(40_960));
    assert!(detalhes.suporta_conversa());
    assert!(detalhes.suporta_raciocinio());
    let pedido = servidor.requisicoes().pop().unwrap();
    assert_eq!(pedido.caminho, "/api/show");
    assert_eq!(
        serde_json::from_str::<Value>(&pedido.corpo).unwrap()["model"],
        "qwen3:8b"
    );
}

#[tokio::test]
async fn r24_modelo_sem_completion_nao_serve_para_conversa() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(r#"{"capabilities":["embedding"]}"#),
    );

    let detalhes = cliente(&servidor.url())
        .detalhes_do_modelo("nomic-embed-text")
        .await
        .unwrap();

    assert_eq!(detalhes.exige_conversa(), Err(ErroOllama::ModeloSemSuporte));
}

#[tokio::test]
async fn r23_modelo_inexistente_no_show_e_modelo_nao_encontrado() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::erro(404, "model 'x' not found"),
    );

    let erro = cliente(&servidor.url())
        .detalhes_do_modelo("x")
        .await
        .unwrap_err();

    assert_eq!(erro, ErroOllama::ModeloNaoEncontrado);
}

#[tokio::test]
async fn consulta_com_json_invalido_da_erro_tipado_e_nao_panico() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota("GET", "/api/tags", Resposta::json("{isto não é json"));

    let erro = cliente(&servidor.url()).listar_modelos().await.unwrap_err();

    assert!(matches!(erro, ErroOllama::RespostaInvalida(_)));
}

// ----- Chat: caminho feliz e formato da conversa -----

#[tokio::test]
async fn conversa_com_streaming_separa_raciocinio_e_conteudo_na_ordem() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_raciocinio("Vou pensar. "),
        &linha_raciocinio("Pronto."),
        &linha_conteudo("Olá"),
        &linha_conteudo(", mundo!"),
        LINHA_FINAL,
    ]))
    .await;

    let (resultado, pedacos) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let resposta = resultado.unwrap();
    assert_eq!(
        pedacos,
        [
            PedacoChat::Raciocinio("Vou pensar. ".into()),
            PedacoChat::Raciocinio("Pronto.".into()),
            PedacoChat::Conteudo("Olá".into()),
            PedacoChat::Conteudo(", mundo!".into()),
        ]
    );
    assert_eq!(resposta.texto.conteudo, "Olá, mundo!");
    assert_eq!(
        resposta.texto.raciocinio.as_deref(),
        Some("Vou pensar. Pronto.")
    );
    assert_eq!(
        resposta.fim,
        MotivoFim::Concluida {
            motivo: Some("stop".into())
        }
    );
    assert!(!resposta.foi_cortada());
}

#[tokio::test]
async fn o_pedido_de_chat_leva_modelo_mensagens_opcoes_e_keep_alive_e_nunca_think() {
    let servidor = servidor_com_chat(Resposta::linhas(&[LINHA_FINAL])).await;

    conversar(&cliente(&servidor.url()), &pedido())
        .await
        .0
        .unwrap();

    let enviado = servidor
        .requisicoes()
        .into_iter()
        .find(|r| r.caminho == "/api/chat")
        .expect("o chat deve ter sido chamado");
    assert_eq!(enviado.metodo, "POST");
    let corpo: Value = serde_json::from_str(&enviado.corpo).unwrap();
    assert_eq!(corpo["model"], "qwen3:8b");
    assert_eq!(corpo["stream"], true);
    assert_eq!(corpo["options"]["temperature"], 0.5);
    assert_eq!(corpo["options"]["num_ctx"], 4096);
    assert_eq!(corpo["keep_alive"], "10m");
    assert_eq!(corpo["messages"][0]["role"], "system");
    assert_eq!(corpo["messages"][0]["content"], "Seja breve.");
    assert_eq!(corpo["messages"][1]["role"], "user");
    assert!(corpo.get("think").is_none());
}

#[tokio::test]
async fn r20_stream_sem_thinking_nao_da_erro_nem_cria_secao_de_raciocinio() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_conteudo("Só resposta."),
        LINHA_FINAL,
    ]))
    .await;

    let (resultado, pedacos) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let resposta = resultado.unwrap();
    assert_eq!(resposta.texto.raciocinio, None);
    assert_eq!(resposta.texto.conteudo, "Só resposta.");
    assert!(pedacos.iter().all(|p| matches!(p, PedacoChat::Conteudo(_))));
}

#[tokio::test]
async fn r25_resposta_cortada_por_limite_e_sinalizada() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_conteudo("texto"),
        r#"{"message":{"role":"assistant","content":""},"done":true,"done_reason":"length"}"#,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    assert!(resultado.unwrap().foi_cortada());
}

#[tokio::test]
async fn ultima_linha_sem_quebra_de_linha_ainda_conta() {
    let servidor = servidor_com_chat(Resposta::Stream(vec![
        Passo::Linha(linha_conteudo("fim")),
        Passo::Bytes(LINHA_FINAL.as_bytes().to_vec()),
        Passo::Terminar,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    assert_eq!(resultado.unwrap().texto.conteudo, "fim");
}

// ----- Chat: stream quebrado (R18) -----

#[tokio::test]
async fn r18_linhas_partidas_em_pedacos_de_rede_sao_reconstruidas_sem_perda() {
    let completo = format!(
        "{}\n{}\n{}\n{}\n",
        linha_raciocinio("pensando em ação"),
        linha_conteudo("Olá, "),
        linha_conteudo("coração! 🎉"),
        LINHA_FINAL
    );
    let bytes = completo.as_bytes();

    // Tamanhos de pedaço que caem no meio do JSON e no meio de caracteres UTF-8.
    for tamanho in [1, 2, 3, 5, 13, 29] {
        let mut passos: Vec<Passo> = bytes
            .chunks(tamanho)
            .map(|pedaco| Passo::Bytes(pedaco.to_vec()))
            .collect();
        passos.push(Passo::Terminar);
        let servidor = servidor_com_chat(Resposta::Stream(passos)).await;

        let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

        let resposta = resultado.unwrap_or_else(|falha| panic!("tamanho {tamanho}: {falha}"));
        assert_eq!(
            resposta.texto.conteudo, "Olá, coração! 🎉",
            "tamanho {tamanho}"
        );
        assert_eq!(
            resposta.texto.raciocinio.as_deref(),
            Some("pensando em ação"),
            "tamanho {tamanho}"
        );
    }
}

// ----- Chat: falhas com texto parcial (R16, R19, R22, R23) -----

#[tokio::test]
async fn r19_json_invalido_no_stream_da_erro_tipado_e_preserva_o_parcial() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_conteudo("Antes"),
        "{isto não é json",
        &linha_conteudo("Depois"),
        LINHA_FINAL,
    ]))
    .await;

    let (resultado, pedacos) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert!(matches!(falha.erro, ErroOllama::RespostaInvalida(_)));
    assert_eq!(falha.parcial.conteudo, "Antes");
    assert_eq!(pedacos, [PedacoChat::Conteudo("Antes".into())]);
    assert!(!falha.erro.mensagem_amigavel().contains("json"));
}

#[tokio::test]
async fn linha_com_formato_inesperado_tambem_e_resposta_invalida() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_conteudo("ok"),
        r#"{"message":{"content":123},"done":false}"#,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert!(matches!(falha.erro, ErroOllama::RespostaInvalida(_)));
    assert_eq!(falha.parcial.conteudo, "ok");
}

#[tokio::test]
async fn r16_ollama_que_cai_no_meio_do_stream_da_erro_tipado_e_preserva_o_parcial() {
    let servidor = servidor_com_chat(Resposta::Stream(vec![
        Passo::Linha(linha_raciocinio("hmm")),
        Passo::Linha(linha_conteudo("Resposta pela")),
        Passo::Cortar,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroOllama::ConexaoInterrompida);
    assert_eq!(falha.parcial.conteudo, "Resposta pela");
    assert_eq!(falha.parcial.raciocinio.as_deref(), Some("hmm"));
    assert!(falha.erro.mensagem_amigavel().contains("conexão"));
}

#[tokio::test]
async fn stream_que_termina_sem_done_e_conexao_interrompida() {
    let servidor = servidor_com_chat(Resposta::linhas(&[&linha_conteudo("incompleta")])).await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroOllama::ConexaoInterrompida);
    assert_eq!(falha.parcial.conteudo, "incompleta");
}

#[tokio::test]
async fn linha_de_erro_no_meio_do_stream_nao_vaza_o_texto_cru() {
    let servidor = servidor_com_chat(Resposta::linhas(&[
        &linha_conteudo("parte"),
        r#"{"error":"llama runner process has terminated: exit status 0xc0000409"}"#,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert!(matches!(falha.erro, ErroOllama::ErroDoOllama(_)));
    assert_eq!(falha.parcial.conteudo, "parte");
    assert!(
        falha.erro.to_string().contains("0xc0000409"),
        "o log guarda o texto técnico"
    );
    let amigavel = falha.erro.mensagem_amigavel();
    assert!(!amigavel.contains("runner") && !amigavel.contains("0xc0000409"));
}

#[tokio::test]
async fn r22_ollama_travado_sem_enviar_bytes_da_tempo_esgotado_e_preserva_o_parcial() {
    let servidor = servidor_com_chat(Resposta::Stream(vec![
        Passo::Linha(linha_conteudo("começo")),
        Passo::Silencio,
    ]))
    .await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroOllama::TempoEsgotado);
    assert_eq!(falha.parcial.conteudo, "começo");
}

#[tokio::test]
async fn ollama_que_nunca_responde_o_pedido_da_tempo_esgotado() {
    let servidor = servidor_com_chat(Resposta::NaoResponder).await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    assert_eq!(resultado.unwrap_err().erro, ErroOllama::TempoEsgotado);
}

#[tokio::test]
async fn r23_modelo_removido_depois_de_escolhido_da_modelo_nao_encontrado() {
    let servidor = servidor_com_chat(Resposta::erro(404, "model 'qwen3:8b' not found")).await;

    let (resultado, pedacos) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroOllama::ModeloNaoEncontrado);
    assert!(pedacos.is_empty());
}

#[tokio::test]
async fn erro_http_do_ollama_vira_erro_tipado_sem_texto_cru_na_mensagem() {
    let servidor = servidor_com_chat(Resposta::erro(500, "segredo interno do servidor")).await;

    let (resultado, _) = conversar(&cliente(&servidor.url()), &pedido()).await;

    let falha = resultado.unwrap_err();
    assert!(matches!(&falha.erro, ErroOllama::ErroDoOllama(d) if d.contains("segredo interno")));
    assert!(!falha.erro.mensagem_amigavel().contains("segredo"));
}

#[tokio::test]
async fn r17_conversar_com_o_ollama_fora_do_ar_da_erro_tipado_sem_texto_parcial() {
    let cliente = cliente_sem_ninguem_na_porta(&endereco_sem_ninguem().await);

    let (resultado, pedacos) = conversar(&cliente, &pedido()).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroOllama::OllamaForaDoAr);
    assert!(falha.parcial.conteudo.is_empty());
    assert!(pedacos.is_empty());
}

// ----- Chat: cancelamento (R21) -----

#[tokio::test]
async fn r21_cancelar_fecha_a_conexao_e_nenhum_texto_chega_depois() {
    let servidor = servidor_com_chat(Resposta::Stream(vec![Passo::LinhaRepetida {
        linha: linha_conteudo("x"),
        intervalo: Duration::from_millis(5),
    }]))
    .await;
    let cliente = cliente(&servidor.url());
    let cancelamento = CancellationToken::new();

    let mut recebidos = 0;
    let resposta = cliente
        .conversar(&pedido(), &cancelamento, &mut |_| {
            recebidos += 1;
            if recebidos == 3 {
                cancelamento.cancel();
            }
        })
        .await
        .expect("cancelar não é erro");

    assert_eq!(resposta.fim, MotivoFim::Cancelada);
    assert_eq!(
        resposta.texto.conteudo, "xxx",
        "mantém o que já tinha chegado"
    );
    assert_eq!(recebidos, 3, "nada é entregue depois do cancelamento");
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await,
        "o servidor deve ver a conexão fechada"
    );
}

#[tokio::test]
async fn cancelar_antes_de_o_ollama_responder_encerra_logo_e_fecha_a_conexao() {
    let servidor = servidor_com_chat(Resposta::NaoResponder).await;
    let cliente = cliente(&servidor.url());
    let cancelamento = CancellationToken::new();
    let para_cancelar = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        para_cancelar.cancel();
    });

    let resposta = cliente
        .conversar(&pedido(), &cancelamento, &mut |_| {})
        .await
        .expect("cancelar não é erro");

    assert_eq!(resposta.fim, MotivoFim::Cancelada);
    assert!(resposta.texto.conteudo.is_empty());
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await
    );
}

// ----- Download de modelo (R09, R10, R11) -----

fn linha_pull(digest: &str, total: u64, baixado: u64) -> String {
    format!(
        r#"{{"status":"pulling {digest}","digest":"{digest}","total":{total},"completed":{baixado}}}"#
    )
}

async fn baixar(
    cliente: &ClienteOllama,
    nome: &str,
) -> (
    Result<ResultadoDownload, ErroOllama>,
    Vec<ProgressoDownload>,
) {
    let mut progressos = Vec::new();
    let resultado = cliente
        .baixar_modelo(nome, &CancellationToken::new(), &mut |p| progressos.push(p))
        .await;
    (resultado, progressos)
}

#[tokio::test]
async fn baixa_o_modelo_com_progresso_ate_o_success() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::linhas(&[
            r#"{"status":"pulling manifest"}"#,
            &linha_pull("aaa", 1_000, 0),
            &linha_pull("bbb", 3_000, 0),
            &linha_pull("aaa", 1_000, 1_000),
            &linha_pull("bbb", 3_000, 1_500),
            &linha_pull("bbb", 3_000, 3_000),
            r#"{"status":"verifying sha256 digest"}"#,
            r#"{"status":"writing manifest"}"#,
            r#"{"status":"success"}"#,
        ]),
    );

    let (resultado, progressos) = baixar(&cliente(&servidor.url()), "llama3.2").await;

    assert_eq!(resultado.unwrap(), ResultadoDownload::Concluido);
    assert_eq!(progressos.first().unwrap().fase, "pulling manifest");
    assert_eq!(
        progressos.first().unwrap().fracao,
        None,
        "antes do total, indeterminado"
    );
    let fracoes: Vec<f64> = progressos.iter().filter_map(|p| p.fracao).collect();
    assert!(
        fracoes.windows(2).all(|par| par[0] <= par[1]),
        "{fracoes:?}"
    );
    assert_eq!(progressos.last().unwrap().fase, "success");
    assert_eq!(progressos.last().unwrap().fracao, Some(1.0));
    assert_eq!(progressos.last().unwrap().total_em_bytes, 4_000);

    let enviado = servidor.requisicoes().pop().unwrap();
    let corpo: Value = serde_json::from_str(&enviado.corpo).unwrap();
    assert_eq!(corpo["model"], "llama3.2");
    assert_eq!(corpo["stream"], true);
}

#[tokio::test]
async fn r09_download_interrompido_no_meio_da_erro_tipado() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![
            Passo::Linha(r#"{"status":"pulling manifest"}"#.into()),
            Passo::Linha(linha_pull("aaa", 1_000, 400)),
            Passo::Cortar,
        ]),
    );

    let (resultado, progressos) = baixar(&cliente(&servidor.url()), "llama3.2").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroOllama::ConexaoInterrompida);
    assert!(erro.mensagem_amigavel().contains("conexão"));
    assert_eq!(progressos.last().unwrap().baixado_em_bytes, 400);
}

#[tokio::test]
async fn download_que_acaba_sem_success_tambem_e_interrompido() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::linhas(&[
            r#"{"status":"pulling manifest"}"#,
            &linha_pull("aaa", 10, 5),
        ]),
    );

    let (resultado, _) = baixar(&cliente(&servidor.url()), "llama3.2").await;

    assert_eq!(resultado.unwrap_err(), ErroOllama::ConexaoInterrompida);
}

#[tokio::test]
async fn r10_modelo_inexistente_no_download_da_erro_amigavel_e_nao_o_erro_cru() {
    // O Ollama pode responder com HTTP 404 ou com uma linha de erro no stream.
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::erro(404, "model 'xyz' not found"),
    );
    let (por_status, _) = baixar(&cliente(&servidor.url()), "xyz").await;
    assert_eq!(por_status.unwrap_err(), ErroOllama::ModeloNaoEncontrado);

    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::linhas(&[
            r#"{"status":"pulling manifest"}"#,
            r#"{"error":"pull model manifest: file does not exist"}"#,
        ]),
    );
    let (por_linha, _) = baixar(&cliente(&servidor.url()), "xyz").await;
    let erro = por_linha.unwrap_err();
    assert!(matches!(erro, ErroOllama::ErroDoOllama(_)));
    assert!(!erro.mensagem_amigavel().contains("manifest"));
}

#[tokio::test]
async fn r11_baixar_com_o_ollama_fora_do_ar_da_erro_tipado() {
    let cliente = cliente_sem_ninguem_na_porta(&endereco_sem_ninguem().await);

    let (resultado, _) = baixar(&cliente, "llama3.2").await;

    assert_eq!(resultado.unwrap_err(), ErroOllama::OllamaForaDoAr);
}

#[tokio::test]
async fn cancelar_o_download_fecha_a_conexao() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![Passo::LinhaRepetida {
            linha: linha_pull("aaa", 1_000_000, 1),
            intervalo: Duration::from_millis(5),
        }]),
    );
    let cliente = cliente(&servidor.url());
    let cancelamento = CancellationToken::new();

    let mut recebidos = 0;
    let resultado = cliente
        .baixar_modelo("llama3.2", &cancelamento, &mut |_| {
            recebidos += 1;
            if recebidos == 2 {
                cancelamento.cancel();
            }
        })
        .await;

    assert_eq!(resultado.unwrap(), ResultadoDownload::Cancelado);
    assert_eq!(recebidos, 2);
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await
    );
}

#[tokio::test]
async fn r21_cancelar_no_meio_de_um_pedaco_com_varias_linhas_nao_entrega_o_resto() {
    // Cinco linhas chegam juntas num único pedaço de rede.
    let juntas: String = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|t| format!("{}\n", linha_conteudo(t)))
        .collect();
    let servidor = servidor_com_chat(Resposta::Stream(vec![
        Passo::Bytes(juntas.into_bytes()),
        Passo::Silencio,
    ]))
    .await;
    let cliente = cliente(&servidor.url());
    let cancelamento = CancellationToken::new();

    let mut recebidos = Vec::new();
    let resposta = cliente
        .conversar(&pedido(), &cancelamento, &mut |pedaco| {
            recebidos.push(pedaco);
            if recebidos.len() == 2 {
                cancelamento.cancel();
            }
        })
        .await
        .expect("cancelar não é erro");

    assert_eq!(resposta.fim, MotivoFim::Cancelada);
    assert_eq!(
        recebidos.len(),
        2,
        "as linhas seguintes do mesmo pedaço não são entregues"
    );
    assert_eq!(resposta.texto.conteudo, "ab");
}
