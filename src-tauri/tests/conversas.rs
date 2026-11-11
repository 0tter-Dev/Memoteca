//! Testes de aceite da E10: conversas e chat com streaming, contra o servidor Ollama falso.
//! Os testes dos cenários do plano de robustez citam o identificador `Rnn` no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::banco::{
    Banco, DadosConversa, DadosProjeto, ErroBanco, EstadoMensagem, Mensagem, Papel,
};
use memoteca_lib::conversas::{
    DesfechoDaGeracao, ErroGeracao, FalhaDeGeracao, ModoDeGeracao, gerar,
    registrar_mensagem_do_usuario,
};
use memoteca_lib::ollama::{ClienteOllama, ErroOllama, Limites, PedacoChat};
use ollama_falso::{OllamaFalso, Passo, Resposta, endereco_sem_ninguem};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

// ----- Apoio -----

fn limites() -> Limites {
    Limites {
        conexao: Duration::from_secs(4),
        consulta: Duration::from_secs(6),
        ociosidade_chat: Duration::from_millis(800),
        ociosidade_pull: Duration::from_millis(800),
    }
}

fn linha_conteudo(texto: &str) -> String {
    format!(
        r#"{{"model":"m","message":{{"role":"assistant","content":{}}},"done":false}}"#,
        serde_json::to_string(texto).unwrap()
    )
}

fn linha_raciocinio(texto: &str) -> String {
    format!(
        r#"{{"model":"m","message":{{"role":"assistant","thinking":{}}},"done":false}}"#,
        serde_json::to_string(texto).unwrap()
    )
}

const FIM: &str =
    r#"{"model":"m","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}"#;

fn resposta(texto: &str) -> Resposta {
    Resposta::linhas(&[&linha_conteudo(texto), FIM])
}

/// Servidor Ollama falso com o modelo `qwen3:8b` instalado, que serve para conversa.
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

fn cliente(url: &str) -> ClienteOllama {
    ClienteOllama::com_endereco(url, limites()).unwrap()
}

/// Um projeto com as instruções dadas e uma conversa nova (modelo `qwen3:8b`).
fn cenario(url: &str, instrucoes: &str) -> Cenario {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "Projeto".into(),
            system_prompt: instrucoes.into(),
            modelo_padrao: Some("qwen3:8b".into()),
        })
        .unwrap();
    let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
    Cenario {
        banco: Mutex::new(banco),
        cliente: cliente(url),
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

    fn conversa(&self) -> memoteca_lib::banco::Conversa {
        self.banco
            .lock()
            .unwrap()
            .obter_conversa(&self.conversa_id)
            .unwrap()
    }

    /// Grava a mensagem do usuário e gera a resposta, como o comando do app faz.
    async fn enviar(
        &self,
        texto: &str,
    ) -> (Result<DesfechoDaGeracao, FalhaDeGeracao>, Vec<PedacoChat>) {
        registrar_mensagem_do_usuario(&self.banco, &self.conversa_id, texto).unwrap();
        self.gerar(
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
        )
        .await
    }

    async fn gerar(
        &self,
        modo: ModoDeGeracao,
        cancelamento: &CancellationToken,
    ) -> (Result<DesfechoDaGeracao, FalhaDeGeracao>, Vec<PedacoChat>) {
        let mut pedacos = Vec::new();
        let resultado = gerar(
            &self.banco,
            &self.cliente,
            &self.conversa_id,
            modo,
            cancelamento,
            &mut |p| pedacos.push(p),
        )
        .await;
        (resultado, pedacos)
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

fn concluida(resultado: Result<DesfechoDaGeracao, FalhaDeGeracao>) -> (Mensagem, bool) {
    match resultado {
        Ok(DesfechoDaGeracao::Concluida {
            mensagem, cortada, ..
        }) => (mensagem, cortada),
        outro => panic!("deveria concluir: {outro:?}"),
    }
}

// ----- Streaming e persistência -----

#[tokio::test]
async fn o_texto_chega_em_streaming_e_a_resposta_e_gravada_com_o_raciocinio() {
    let servidor = servidor(Resposta::linhas(&[
        &linha_raciocinio("Vou pensar. "),
        &linha_raciocinio("Pronto."),
        &linha_conteudo("Olá"),
        &linha_conteudo(", mundo!"),
        FIM,
    ]))
    .await;
    let c = cenario(&servidor.url(), "");

    let (resultado, pedacos) = c.enviar("Oi").await;

    let (mensagem, cortada) = concluida(resultado);
    assert!(!cortada);
    assert_eq!(
        pedacos,
        [
            PedacoChat::Raciocinio("Vou pensar. ".into()),
            PedacoChat::Raciocinio("Pronto.".into()),
            PedacoChat::Conteudo("Olá".into()),
            PedacoChat::Conteudo(", mundo!".into()),
        ]
    );
    assert_eq!(mensagem.papel, Papel::Assistente);
    assert_eq!(mensagem.conteudo, "Olá, mundo!");
    assert_eq!(mensagem.raciocinio.as_deref(), Some("Vou pensar. Pronto."));
    assert_eq!(mensagem.estado, EstadoMensagem::Completa);

    let todas = c.mensagens();
    assert_eq!(todas.len(), 2);
    assert_eq!(
        (todas[0].papel, todas[0].conteudo.as_str()),
        (Papel::Usuario, "Oi")
    );
    assert_eq!(todas[1], mensagem);
}

#[tokio::test]
async fn uma_resposta_sem_thinking_nao_tem_raciocinio_gravado() {
    let servidor = servidor(resposta("Só resposta.")).await;
    let c = cenario(&servidor.url(), "");

    let (resultado, pedacos) = c.enviar("Oi").await;

    let (mensagem, _) = concluida(resultado);
    assert_eq!(mensagem.raciocinio, None);
    assert!(pedacos.iter().all(|p| matches!(p, PedacoChat::Conteudo(_))));
}

#[tokio::test]
async fn o_historico_continua_depois_de_fechar_e_reabrir() {
    let servidor = servidor(Resposta::linhas(&[
        &linha_raciocinio("pensando"),
        &linha_conteudo("Resposta guardada."),
        FIM,
    ]))
    .await;
    let pasta = std::env::temp_dir().join(format!("memoteca-conversas-{}", novo_id()));
    fs::create_dir_all(&pasta).unwrap();
    let arquivo: PathBuf = pasta.join("memoteca.db");

    let (projeto_id, conversa_id) = {
        let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
        let projeto = banco
            .criar_projeto(&DadosProjeto {
                nome: "P".into(),
                system_prompt: String::new(),
                modelo_padrao: Some("qwen3:8b".into()),
            })
            .unwrap();
        let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
        let banco = Mutex::new(banco);
        registrar_mensagem_do_usuario(&banco, &conversa.id, "Pergunta?").unwrap();
        let resultado = gerar(
            &banco,
            &cliente(&servidor.url()),
            &conversa.id,
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .await;
        concluida(resultado);
        (projeto.id, conversa.id)
    };

    let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
    let mensagens = banco.listar_mensagens(&conversa_id).unwrap();
    assert_eq!(mensagens.len(), 2);
    assert_eq!(mensagens[0].conteudo, "Pergunta?");
    assert_eq!(mensagens[1].conteudo, "Resposta guardada.");
    assert_eq!(mensagens[1].raciocinio.as_deref(), Some("pensando"));
    assert_eq!(
        banco.listar_conversas_do_projeto(&projeto_id).unwrap()[0].titulo,
        "Pergunta?"
    );
    drop(banco);
    let _ = fs::remove_dir_all(&pasta);
}

// ----- Contexto: o system prompt em cada mensagem -----

#[tokio::test]
async fn o_system_prompt_do_projeto_entra_em_cada_mensagem_enviada_e_nao_so_na_primeira() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "Responda sempre em português.");

    for pergunta in ["Primeira", "Segunda", "Terceira"] {
        let (resultado, _) = c.enviar(pergunta).await;
        concluida(resultado);
    }

    let corpos = corpos_do_chat(&servidor);
    assert_eq!(corpos.len(), 3);
    for (indice, corpo) in corpos.iter().enumerate() {
        let mensagens = corpo["messages"].as_array().unwrap();
        assert_eq!(mensagens[0]["role"], "system", "requisição {indice}");
        assert_eq!(
            mensagens[0]["content"], "Responda sempre em português.",
            "requisição {indice}"
        );
        // Uma única mensagem system, e o histórico cresce a cada envio.
        assert_eq!(
            mensagens.iter().filter(|m| m["role"] == "system").count(),
            1,
            "requisição {indice}"
        );
        assert_eq!(mensagens.len(), 1 + 2 * indice + 1, "requisição {indice}");
    }
    let ultima = corpos[2]["messages"].as_array().unwrap();
    assert_eq!(ultima[1]["content"], "Primeira");
    assert_eq!(ultima[2]["role"], "assistant");
    assert_eq!(ultima[5]["content"], "Terceira");
}

#[tokio::test]
async fn sem_instrucoes_nao_vai_mensagem_system_e_o_raciocinio_nao_volta_no_historico() {
    let servidor = servidor(Resposta::linhas(&[
        &linha_raciocinio("segredo do raciocínio"),
        &linha_conteudo("Resposta."),
        FIM,
    ]))
    .await;
    let c = cenario(&servidor.url(), "   ");

    concluida(c.enviar("Um").await.0);
    concluida(c.enviar("Dois").await.0);

    let corpos = corpos_do_chat(&servidor);
    let segunda = corpos[1]["messages"].as_array().unwrap();
    assert!(segunda.iter().all(|m| m["role"] != "system"));
    assert_eq!(segunda.len(), 3);
    assert!(!corpos[1].to_string().contains("segredo do raciocínio"));
    assert!(corpos[1].get("think").is_none());
}

// ----- Modelo e parâmetros por conversa -----

#[tokio::test]
async fn cada_conversa_envia_e_guarda_o_proprio_modelo_e_os_proprios_parametros() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "");
    let outra = {
        let banco = c.banco.lock().unwrap();
        let outra = banco.criar_conversa_nova(&c.projeto_id).unwrap();
        banco
            .atualizar_conversa(
                &outra.id,
                &DadosConversa {
                    titulo: "Outra".into(),
                    modelo: Some("llama3.2:3b".into()),
                    temperature: Some(1.5),
                    num_ctx: Some(2048),
                    keep_alive: Some("-1".into()),
                },
            )
            .unwrap();
        outra
    };

    // A conversa padrão usa os valores padrão.
    concluida(c.enviar("Oi").await.0);
    // A outra usa os próprios.
    let banco_da_outra = &c.banco;
    registrar_mensagem_do_usuario(banco_da_outra, &outra.id, "Olá").unwrap();
    gerar(
        banco_da_outra,
        &c.cliente,
        &outra.id,
        ModoDeGeracao::ResponderUltimaMensagem,
        &CancellationToken::new(),
        &mut |_| {},
    )
    .await
    .unwrap();

    let corpos = corpos_do_chat(&servidor);
    assert_eq!(corpos[0]["model"], "qwen3:8b");
    assert_eq!(corpos[0]["options"]["temperature"], 0.7);
    assert_eq!(corpos[0]["options"]["num_ctx"], 8192);
    assert_eq!(corpos[0]["keep_alive"], "5m");
    assert_eq!(corpos[1]["model"], "llama3.2:3b");
    assert_eq!(corpos[1]["options"]["temperature"], 1.5);
    assert_eq!(corpos[1]["options"]["num_ctx"], 2048);
    assert_eq!(corpos[1]["keep_alive"], -1);

    // Mudar uma não alterou a outra.
    let padrao = c.conversa();
    assert_eq!(padrao.modelo.as_deref(), Some("qwen3:8b"));
    assert_eq!(padrao.temperature, Some(0.7));
    assert_eq!(padrao.num_ctx, Some(8192));
    assert_eq!(padrao.keep_alive.as_deref(), Some("5m"));
}

#[tokio::test]
async fn o_contexto_pedido_e_limitado_a_janela_maxima_do_modelo() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion"],"model_info":{"llama.context_length":4096}}"#,
        ),
    );
    servidor.rota("POST", "/api/chat", resposta("ok"));
    let c = cenario(&servidor.url(), "");

    concluida(c.enviar("Oi").await.0);

    let corpos = corpos_do_chat(&servidor);
    assert_eq!(
        corpos[0]["options"]["num_ctx"], 4096,
        "8192 pedido, 4096 de janela"
    );
}

// ----- Falhas: Ollama fora do ar (R17), queda no meio (R16) -----

#[tokio::test]
async fn r17_com_o_ollama_fora_do_ar_a_mensagem_do_usuario_nao_se_perde_e_tentar_de_novo_funciona()
{
    let url = endereco_sem_ninguem().await;
    let c = cenario(&url, "");
    let c = Cenario {
        cliente: ClienteOllama::com_endereco(
            &url,
            Limites {
                conexao: Duration::from_secs(4),
                consulta: Duration::from_secs(6),
                ociosidade_chat: Duration::from_secs(6),
                ociosidade_pull: Duration::from_secs(6),
            },
        )
        .unwrap(),
        ..c
    };

    let (resultado, pedacos) = c.enviar("Não pode se perder").await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroGeracao::Ollama(ErroOllama::OllamaForaDoAr));
    assert!(falha.mensagem_parcial.is_none());
    assert!(falha.erro.mensagem_amigavel().contains("Ollama"));
    assert!(pedacos.is_empty());
    let mensagens = c.mensagens();
    assert_eq!(mensagens.len(), 1, "só a mensagem do usuário, sem resposta");
    assert_eq!(mensagens[0].conteudo, "Não pode se perder");
    assert_eq!(mensagens[0].estado, EstadoMensagem::Completa);

    // O Ollama volta, e "Tentar de novo" responde à mesma mensagem, sem duplicá-la.
    let endereco = url.trim_start_matches("http://").to_owned();
    let servidor = OllamaFalso::iniciar_em(&endereco).await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(r#"{"capabilities":["completion"]}"#),
    );
    servidor.rota("POST", "/api/chat", resposta("Voltei."));
    let (segunda, _) = c
        .gerar(
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
        )
        .await;

    let (resposta, _) = concluida(segunda);
    assert_eq!(resposta.conteudo, "Voltei.");
    let mensagens = c.mensagens();
    assert_eq!(mensagens.len(), 2);
    assert_eq!(mensagens[0].conteudo, "Não pode se perder");
}

#[tokio::test]
async fn r16_se_o_ollama_cai_no_meio_o_que_chegou_fica_guardado_como_interrompido() {
    let servidor = servidor(Resposta::Stream(vec![
        Passo::Linha(linha_raciocinio("hmm")),
        Passo::Linha(linha_conteudo("Resposta pela")),
        Passo::Cortar,
    ]))
    .await;
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Pergunta").await;

    let falha = resultado.unwrap_err();
    assert_eq!(
        falha.erro,
        ErroGeracao::Ollama(ErroOllama::ConexaoInterrompida)
    );
    let parcial = falha.mensagem_parcial.expect("o parcial deve ser guardado");
    assert_eq!(parcial.conteudo, "Resposta pela");
    assert_eq!(parcial.raciocinio.as_deref(), Some("hmm"));
    assert_eq!(parcial.estado, EstadoMensagem::Interrompida);
    let mensagens = c.mensagens();
    assert_eq!(mensagens.len(), 2);
    assert_eq!(
        mensagens[0].conteudo, "Pergunta",
        "a pergunta do usuário continua"
    );
    assert_eq!(mensagens[1].estado, EstadoMensagem::Interrompida);

    // Regenerar troca a resposta interrompida pela completa.
    servidor.rota("POST", "/api/chat", resposta("Resposta inteira."));
    let (regerada, _) = c
        .gerar(
            ModoDeGeracao::RegenerarUltimaResposta,
            &CancellationToken::new(),
        )
        .await;
    let (nova, _) = concluida(regerada);
    assert_eq!(nova.id, mensagens[1].id);
    assert_eq!(nova.conteudo, "Resposta inteira.");
    assert_eq!(nova.estado, EstadoMensagem::Completa);
    assert_eq!(c.mensagens().len(), 2);
}

#[tokio::test]
async fn queda_sem_nenhum_texto_nao_guarda_resposta_alguma() {
    let servidor = servidor(Resposta::Stream(vec![Passo::Cortar])).await;
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Pergunta").await;

    assert!(resultado.unwrap_err().mensagem_parcial.is_none());
    assert_eq!(c.mensagens().len(), 1);
}

// ----- Cancelamento -----

#[tokio::test]
async fn cancelar_com_texto_guarda_o_que_chegou_como_interrompido() {
    let servidor = servidor(Resposta::Stream(vec![
        Passo::Linha(linha_conteudo("Parte 1. ")),
        Passo::Linha(linha_conteudo("Parte 2.")),
        Passo::Silencio,
    ]))
    .await;
    let c = cenario(&servidor.url(), "");
    registrar_mensagem_do_usuario(&c.banco, &c.conversa_id, "Pergunta").unwrap();
    let cancelamento = CancellationToken::new();
    let mut recebidos = 0;

    let resultado = gerar(
        &c.banco,
        &c.cliente,
        &c.conversa_id,
        ModoDeGeracao::ResponderUltimaMensagem,
        &cancelamento,
        &mut |_| {
            recebidos += 1;
            if recebidos == 2 {
                cancelamento.cancel();
            }
        },
    )
    .await
    .unwrap();

    let DesfechoDaGeracao::Cancelada { mensagem } = resultado else {
        panic!("deveria ter sido cancelada");
    };
    let mensagem = mensagem.expect("o texto que chegou é guardado");
    assert_eq!(mensagem.conteudo, "Parte 1. Parte 2.");
    assert_eq!(mensagem.estado, EstadoMensagem::Interrompida);
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await
    );
}

#[tokio::test]
async fn cancelar_antes_de_chegar_texto_nao_guarda_nada() {
    let servidor = servidor(Resposta::NaoResponder).await;
    let c = cenario(&servidor.url(), "");
    registrar_mensagem_do_usuario(&c.banco, &c.conversa_id, "Pergunta").unwrap();
    let cancelamento = CancellationToken::new();
    let para_cancelar = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        para_cancelar.cancel();
    });

    let (resultado, _) = c
        .gerar(ModoDeGeracao::ResponderUltimaMensagem, &cancelamento)
        .await;

    assert_eq!(
        resultado.unwrap(),
        DesfechoDaGeracao::Cancelada { mensagem: None }
    );
    assert_eq!(c.mensagens().len(), 1);
}

// ----- Regenerar (R: substitui, não acrescenta) -----

#[tokio::test]
async fn regenerar_descarta_a_resposta_antiga_e_deixa_so_a_nova() {
    let servidor = servidor(resposta("Resposta antiga.")).await;
    let c = cenario(&servidor.url(), "Instruções.");
    let (antiga, _) = concluida(c.enviar("Pergunta").await.0);

    servidor.rota("POST", "/api/chat", resposta("Resposta nova."));
    let (resultado, _) = c
        .gerar(
            ModoDeGeracao::RegenerarUltimaResposta,
            &CancellationToken::new(),
        )
        .await;

    let (nova, _) = concluida(resultado);
    let mensagens = c.mensagens();
    assert_eq!(mensagens.len(), 2, "continua uma pergunta e uma resposta");
    assert_eq!(mensagens[1].conteudo, "Resposta nova.");
    assert_eq!(nova.id, antiga.id, "mesma posição, mesmo ID");
    assert!(
        mensagens.iter().all(|m| !m.conteudo.contains("antiga")),
        "a resposta antiga não existe mais"
    );

    // A regeneração não reenvia a resposta antiga como contexto.
    let corpos = corpos_do_chat(&servidor);
    let enviadas = corpos[1]["messages"].as_array().unwrap();
    assert_eq!(enviadas.len(), 2, "instruções e a pergunta");
    assert_eq!(enviadas[0]["role"], "system");
    assert_eq!(enviadas[1]["content"], "Pergunta");
}

#[tokio::test]
async fn regenerar_com_falha_mantem_a_resposta_antiga() {
    let servidor = servidor(resposta("Resposta boa.")).await;
    let c = cenario(&servidor.url(), "");
    concluida(c.enviar("Pergunta").await.0);

    servidor.rota(
        "POST",
        "/api/chat",
        Resposta::Stream(vec![
            Passo::Linha(linha_conteudo("Começo da nova")),
            Passo::Cortar,
        ]),
    );
    let (resultado, _) = c
        .gerar(
            ModoDeGeracao::RegenerarUltimaResposta,
            &CancellationToken::new(),
        )
        .await;

    let falha = resultado.unwrap_err();
    assert!(falha.mensagem_parcial.is_none());
    let mensagens = c.mensagens();
    assert_eq!(mensagens.len(), 2);
    assert_eq!(mensagens[1].conteudo, "Resposta boa.");
    assert_eq!(mensagens[1].estado, EstadoMensagem::Completa);
}

#[tokio::test]
async fn regenerar_e_cancelar_sem_texto_mantem_a_resposta_antiga() {
    let servidor = servidor(resposta("Resposta boa.")).await;
    let c = cenario(&servidor.url(), "");
    concluida(c.enviar("Pergunta").await.0);

    servidor.rota("POST", "/api/chat", Resposta::NaoResponder);
    let cancelamento = CancellationToken::new();
    let para_cancelar = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        para_cancelar.cancel();
    });
    let (resultado, _) = c
        .gerar(ModoDeGeracao::RegenerarUltimaResposta, &cancelamento)
        .await;

    assert_eq!(
        resultado.unwrap(),
        DesfechoDaGeracao::Cancelada { mensagem: None }
    );
    assert_eq!(c.mensagens()[1].conteudo, "Resposta boa.");
}

#[tokio::test]
async fn nao_se_responde_a_uma_resposta_nem_se_regenera_uma_pergunta() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "");

    // Sem mensagens: nada para responder nem regenerar.
    let (sem_nada, _) = c
        .gerar(
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(sem_nada.unwrap_err().erro, ErroGeracao::NadaParaResponder);

    // A última mensagem é do usuário: não há resposta para regenerar.
    registrar_mensagem_do_usuario(&c.banco, &c.conversa_id, "Pergunta").unwrap();
    let (regenerar, _) = c
        .gerar(
            ModoDeGeracao::RegenerarUltimaResposta,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(regenerar.unwrap_err().erro, ErroGeracao::NadaParaRegenerar);

    // A última é uma resposta: não há pergunta sem resposta.
    concluida(
        c.gerar(
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
        )
        .await
        .0,
    );
    let (responder, _) = c
        .gerar(
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(responder.unwrap_err().erro, ErroGeracao::NadaParaResponder);
}

// ----- Modelo da conversa -----

#[tokio::test]
async fn sem_modelo_escolhido_o_erro_e_amigavel_e_a_mensagem_fica() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "");
    c.banco
        .lock()
        .unwrap()
        .atualizar_conversa(
            &c.conversa_id,
            &DadosConversa {
                titulo: "Sem modelo".into(),
                modelo: None,
                temperature: None,
                num_ctx: None,
                keep_alive: None,
            },
        )
        .unwrap();

    let (resultado, _) = c.enviar("Oi").await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroGeracao::SemModelo);
    assert!(falha.erro.mensagem_amigavel().contains("Escolha um modelo"));
    assert_eq!(c.mensagens().len(), 1);
    assert!(corpos_do_chat(&servidor).is_empty(), "nem chegou ao Ollama");
}

#[tokio::test]
async fn r23_modelo_removido_da_erro_amigavel_sem_perder_a_mensagem() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::erro(404, "model 'qwen3:8b' not found"),
    );
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Oi").await;

    let falha = resultado.unwrap_err();
    assert_eq!(
        falha.erro,
        ErroGeracao::Ollama(ErroOllama::ModeloNaoEncontrado)
    );
    assert!(!falha.erro.mensagem_amigavel().contains("not found"));
    assert_eq!(c.mensagens().len(), 1);
}

#[tokio::test]
async fn r24_modelo_sem_suporte_a_conversa_nao_e_usado() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(r#"{"capabilities":["embedding"]}"#),
    );
    servidor.rota("POST", "/api/chat", resposta("não deveria chegar"));
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Oi").await;

    assert_eq!(
        resultado.unwrap_err().erro,
        ErroGeracao::Ollama(ErroOllama::ModeloSemSuporte)
    );
    assert!(corpos_do_chat(&servidor).is_empty());
}

// ----- Respostas especiais -----

#[tokio::test]
async fn r25_resposta_cortada_por_limite_e_sinalizada_mas_gravada() {
    let servidor = servidor(Resposta::linhas(&[
        &linha_conteudo("texto longo"),
        r#"{"message":{"role":"assistant","content":""},"done":true,"done_reason":"length"}"#,
    ]))
    .await;
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Oi").await;

    let (mensagem, cortada) = concluida(resultado);
    assert!(cortada);
    assert_eq!(mensagem.conteudo, "texto longo");
}

#[tokio::test]
async fn resposta_vazia_do_modelo_da_erro_amigavel_e_nao_grava_nada() {
    let servidor = servidor(Resposta::linhas(&[FIM])).await;
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Oi").await;

    assert_eq!(resultado.unwrap_err().erro, ErroGeracao::RespostaVazia);
    assert_eq!(c.mensagens().len(), 1);
}

#[tokio::test]
async fn erro_de_json_no_stream_guarda_o_que_chegou_como_interrompido() {
    let servidor = servidor(Resposta::linhas(&[
        &linha_conteudo("Antes"),
        "{isto não é json",
    ]))
    .await;
    let c = cenario(&servidor.url(), "");

    let (resultado, _) = c.enviar("Oi").await;

    let falha = resultado.unwrap_err();
    assert!(matches!(
        falha.erro,
        ErroGeracao::Ollama(ErroOllama::RespostaInvalida(_))
    ));
    assert_eq!(falha.mensagem_parcial.unwrap().conteudo, "Antes");
}

// ----- Mensagem do usuário e título -----

#[tokio::test]
async fn a_primeira_mensagem_vira_o_titulo_truncado_e_o_titulo_editado_nunca_e_sobrescrito() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "");
    assert_eq!(c.conversa().titulo, "Nova conversa");

    c.enviar("Explique como funciona o ciclo de vida de um processo no Windows")
        .await
        .0
        .unwrap();

    assert_eq!(
        c.conversa().titulo,
        "Explique como funciona o ciclo de vida…"
    );

    // O usuário edita o título, e as mensagens seguintes não o mudam.
    let conversa = c.conversa();
    c.banco
        .lock()
        .unwrap()
        .atualizar_conversa(
            &c.conversa_id,
            &DadosConversa {
                titulo: "Meu título".into(),
                modelo: conversa.modelo,
                temperature: conversa.temperature,
                num_ctx: conversa.num_ctx,
                keep_alive: conversa.keep_alive,
            },
        )
        .unwrap();
    c.enviar("Outra pergunta qualquer").await.0.unwrap();
    assert_eq!(c.conversa().titulo, "Meu título");
}

#[tokio::test]
async fn o_titulo_editado_antes_da_primeira_mensagem_tambem_e_respeitado() {
    let servidor = servidor(resposta("ok")).await;
    let c = cenario(&servidor.url(), "");
    let conversa = c.conversa();
    c.banco
        .lock()
        .unwrap()
        .atualizar_conversa(
            &c.conversa_id,
            &DadosConversa {
                titulo: "Já tem nome".into(),
                modelo: conversa.modelo,
                temperature: conversa.temperature,
                num_ctx: conversa.num_ctx,
                keep_alive: conversa.keep_alive,
            },
        )
        .unwrap();

    c.enviar("Primeira mensagem").await.0.unwrap();

    assert_eq!(c.conversa().titulo, "Já tem nome");
}

#[test]
fn mensagem_em_branco_ou_enorme_e_recusada_sem_gravar() {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "P".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
    let banco = Mutex::new(banco);

    for texto in ["", "   ", "\n\t "] {
        let Err(erro) = registrar_mensagem_do_usuario(&banco, &conversa.id, texto) else {
            panic!("deveria ser recusada: {texto:?}");
        };
        assert!(matches!(erro, ErroBanco::Rejeitado(_)));
        assert!(erro.mensagem_amigavel().contains("Escreva uma mensagem"));
    }
    let enorme = "x".repeat(100_001);
    assert!(registrar_mensagem_do_usuario(&banco, &conversa.id, &enorme).is_err());
    assert!(
        banco
            .lock()
            .unwrap()
            .listar_mensagens(&conversa.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        banco
            .lock()
            .unwrap()
            .obter_conversa(&conversa.id)
            .unwrap()
            .titulo,
        "Nova conversa"
    );
}

#[test]
fn as_conversas_ficam_em_ordem_da_mais_recente_para_a_mais_antiga() {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "P".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let primeira = banco.criar_conversa_nova(&projeto.id).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let segunda = banco.criar_conversa_nova(&projeto.id).unwrap();
    let ids = |banco: &Banco| -> Vec<String> {
        banco
            .listar_conversas_do_projeto(&projeto.id)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect()
    };
    assert_eq!(ids(&banco), [segunda.id.clone(), primeira.id.clone()]);

    // Mexer na mais antiga a leva para o topo.
    std::thread::sleep(Duration::from_millis(5));
    let banco = Mutex::new(banco);
    registrar_mensagem_do_usuario(&banco, &primeira.id, "Oi").unwrap();

    assert_eq!(ids(&banco.lock().unwrap()), [primeira.id, segunda.id]);
}

// ----- R33: fechar o app no meio de uma geração -----

/// Fechar o app (processo encerrado) no meio de uma resposta deixa o banco consistente na
/// próxima abertura: a mensagem do usuário está lá, não há resposta pela metade nem
/// duplicada, e dá para responder de novo.
#[tokio::test]
async fn r33_fechar_o_app_no_meio_da_geracao_deixa_o_estado_consistente_e_da_para_responder_de_novo()
 {
    let servidor_lento = servidor(Resposta::Stream(vec![
        Passo::Linha(linha_conteudo("Resposta que nunca ")),
        Passo::Linha(linha_conteudo("vai terminar")),
        Passo::Silencio,
    ]))
    .await;
    let pasta = std::env::temp_dir().join(format!("memoteca-r33-{}", novo_id()));
    fs::create_dir_all(&pasta).unwrap();
    let arquivo: PathBuf = pasta.join("memoteca.db");

    // Primeira "sessão" do app: o usuário envia e o processo é encerrado no meio.
    let conversa_id = {
        let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
        let projeto = banco
            .criar_projeto(&DadosProjeto {
                nome: "P".into(),
                system_prompt: String::new(),
                modelo_padrao: Some("qwen3:8b".into()),
            })
            .unwrap();
        let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
        let banco = Mutex::new(banco);
        registrar_mensagem_do_usuario(&banco, &conversa.id, "Pergunta longa").unwrap();
        let cancelamento = CancellationToken::new();
        let cliente_lento = cliente(&servidor_lento.url());
        let mut recebidos = 0;
        let mut contar = |_: PedacoChat| recebidos += 1;
        let geracao = gerar(
            &banco,
            &cliente_lento,
            &conversa.id,
            ModoDeGeracao::ResponderUltimaMensagem,
            &cancelamento,
            &mut contar,
        );
        // O tempo acaba com a resposta pela metade: a tarefa é abandonada de repente, sem
        // passar por nenhum tratamento de cancelamento ou de falha.
        let fim = tokio::time::timeout(Duration::from_millis(500), geracao).await;
        assert!(fim.is_err(), "a geração não deveria terminar sozinha");
        assert!(recebidos >= 1, "o texto já estava chegando");
        conversa.id
    };

    // Segunda "sessão": o banco abre normalmente e está íntegro.
    let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
    let mensagens = banco.listar_mensagens(&conversa_id).unwrap();
    assert_eq!(mensagens.len(), 1, "só a mensagem do usuário");
    assert_eq!(mensagens[0].conteudo, "Pergunta longa");
    assert_eq!(mensagens[0].papel, Papel::Usuario);
    let integridade: String = rusqlite::Connection::open(&arquivo)
        .unwrap()
        .query_row("PRAGMA integrity_check", [], |linha| linha.get(0))
        .unwrap();
    assert_eq!(integridade, "ok");

    // E o usuário consegue pedir a resposta de novo ("Gerar resposta"), uma só vez.
    let servidor_bom = servidor(resposta("Agora sim.")).await;
    let banco = Mutex::new(banco);
    let resultado = gerar(
        &banco,
        &cliente(&servidor_bom.url()),
        &conversa_id,
        ModoDeGeracao::ResponderUltimaMensagem,
        &CancellationToken::new(),
        &mut |_| {},
    )
    .await;
    let (resposta_final, _) = concluida(resultado);
    assert_eq!(resposta_final.conteudo, "Agora sim.");
    let todas = banco
        .lock()
        .unwrap()
        .listar_mensagens(&conversa_id)
        .unwrap();
    assert_eq!(todas.len(), 2, "sem resposta duplicada nem sobrando");
    drop(banco);
    let _ = fs::remove_dir_all(&pasta);
}
