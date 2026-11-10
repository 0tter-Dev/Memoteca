//! Testes de aceite da E11: memória manual (projeto e global) e injeção no contexto, contra o
//! servidor Ollama falso. Os critérios de falha do plano levam "(falha)" no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::fs;
use std::sync::Mutex;
use std::time::Duration;

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::banco::{
    Banco, DadosConversa, DadosProjeto, ErroBanco, MEMORIA_MAXIMA, Mensagem,
};
use memoteca_lib::conversas::{
    AvisoDeContexto, DesfechoDaGeracao, FalhaDeGeracao, ModoDeGeracao, gerar,
    registrar_mensagem_do_usuario,
};
use memoteca_lib::ollama::{ClienteOllama, Limites};
use ollama_falso::{OllamaFalso, Resposta};
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

const RESPOSTA: &str = concat!(
    r#"{"model":"m","message":{"role":"assistant","content":"ok"},"done":false}"#,
    "\n",
    r#"{"model":"m","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}"#,
);

async fn servidor() -> OllamaFalso {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/show",
        Resposta::json(
            r#"{"capabilities":["completion"],"model_info":{"qwen3.context_length":40960}}"#,
        ),
    );
    servidor.rota(
        "POST",
        "/api/chat",
        Resposta::linhas(&RESPOSTA.lines().collect::<Vec<_>>()),
    );
    servidor
}

struct Ambiente {
    banco: Mutex<Banco>,
    cliente: ClienteOllama,
}

impl Ambiente {
    fn novo(url: &str) -> Self {
        Self {
            banco: Mutex::new(Banco::em_memoria().unwrap()),
            cliente: ClienteOllama::com_endereco(url, limites()).unwrap(),
        }
    }

    fn banco(&self) -> std::sync::MutexGuard<'_, Banco> {
        self.banco.lock().unwrap()
    }

    /// Um projeto com instruções e uma conversa nova. Devolve os dois IDs.
    fn projeto(&self, nome: &str, instrucoes: &str) -> (String, String) {
        let banco = self.banco();
        let projeto = banco
            .criar_projeto(&DadosProjeto {
                nome: nome.into(),
                system_prompt: instrucoes.into(),
                modelo_padrao: Some("qwen3:8b".into()),
            })
            .unwrap();
        let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
        (projeto.id, conversa.id)
    }

    async fn enviar(
        &self,
        conversa_id: &str,
        texto: &str,
    ) -> Result<DesfechoDaGeracao, FalhaDeGeracao> {
        registrar_mensagem_do_usuario(&self.banco, conversa_id, texto).unwrap();
        gerar(
            &self.banco,
            &self.cliente,
            conversa_id,
            ModoDeGeracao::ResponderUltimaMensagem,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .await
    }

    fn mensagens(&self, conversa_id: &str) -> Vec<Mensagem> {
        self.banco().listar_mensagens(conversa_id).unwrap()
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

/// O texto da mensagem `system` de um pedido, se houver.
fn system_de(corpo: &Value) -> Option<String> {
    let mensagens = corpo["messages"].as_array().unwrap();
    let primeira = &mensagens[0];
    (primeira["role"] == "system").then(|| primeira["content"].as_str().unwrap().to_owned())
}

fn concluida(resultado: Result<DesfechoDaGeracao, FalhaDeGeracao>) -> Option<AvisoDeContexto> {
    match resultado {
        Ok(DesfechoDaGeracao::Concluida { contexto, .. }) => contexto,
        outro => panic!("deveria concluir: {outro:?}"),
    }
}

// ----- Escopo: global em qualquer projeto, de projeto só no seu -----

#[tokio::test]
async fn memoria_global_entra_em_qualquer_projeto_e_a_de_projeto_so_no_seu() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (projeto_a, conversa_a) = a.projeto("A", "");
    let (projeto_b, conversa_b) = a.projeto("B", "");
    a.banco().criar_memoria_global("Mora em Recife").unwrap();
    a.banco()
        .criar_memoria_de_projeto(&projeto_a, "Só do projeto A")
        .unwrap();
    a.banco()
        .criar_memoria_de_projeto(&projeto_b, "Só do projeto B")
        .unwrap();

    concluida(a.enviar(&conversa_a, "Oi").await);
    concluida(a.enviar(&conversa_b, "Oi").await);

    let corpos = corpos_do_chat(&servidor);
    let sistema_a = system_de(&corpos[0]).unwrap();
    let sistema_b = system_de(&corpos[1]).unwrap();
    assert!(sistema_a.contains("Mora em Recife"));
    assert!(sistema_a.contains("Só do projeto A"));
    assert!(!sistema_a.contains("Só do projeto B"));
    assert!(sistema_b.contains("Mora em Recife"));
    assert!(sistema_b.contains("Só do projeto B"));
    assert!(!sistema_b.contains("Só do projeto A"));
}

#[tokio::test]
async fn a_ordem_do_system_e_instrucoes_globais_projeto_e_vale_em_todo_envio() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (projeto, conversa) = a.projeto("A", "Responda em português.");
    a.banco().criar_memoria_global("Global 1").unwrap();
    a.banco().criar_memoria_global("Global 2").unwrap();
    a.banco()
        .criar_memoria_de_projeto(&projeto, "Projeto 1")
        .unwrap();

    for pergunta in ["Um", "Dois", "Três"] {
        concluida(a.enviar(&conversa, pergunta).await);
    }

    let esperado = "Responda em português.\n\n\
                    Memórias globais (valem para qualquer projeto):\n- Global 1\n- Global 2\n\n\
                    Memórias deste projeto:\n- Projeto 1";
    let corpos = corpos_do_chat(&servidor);
    assert_eq!(corpos.len(), 3);
    for (indice, corpo) in corpos.iter().enumerate() {
        assert_eq!(
            system_de(corpo).as_deref(),
            Some(esperado),
            "envio {indice}"
        );
        // Continua sendo uma única mensagem system.
        let mensagens = corpo["messages"].as_array().unwrap();
        assert_eq!(
            mensagens.iter().filter(|m| m["role"] == "system").count(),
            1
        );
    }
}

#[tokio::test]
async fn sem_instrucoes_so_as_memorias_formam_o_system() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (_, conversa) = a.projeto("A", "");
    a.banco().criar_memoria_global("Fato").unwrap();

    concluida(a.enviar(&conversa, "Oi").await);

    assert_eq!(
        system_de(&corpos_do_chat(&servidor)[0]).as_deref(),
        Some("Memórias globais (valem para qualquer projeto):\n- Fato")
    );
}

// ----- Editar e excluir: só as gerações seguintes (falha) -----

#[tokio::test]
async fn editar_uma_memoria_muda_so_as_geracoes_seguintes_e_o_que_ja_foi_gerado_fica_igual() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (_, conversa) = a.projeto("A", "");
    let memoria = a.banco().criar_memoria_global("Gosta de café").unwrap();

    concluida(a.enviar(&conversa, "Primeira").await);
    let antes = a.mensagens(&conversa);

    a.banco()
        .atualizar_memoria(&memoria.id, "Gosta de chá")
        .unwrap();

    // Nada do que já existia mudou.
    assert_eq!(a.mensagens(&conversa), antes);

    concluida(a.enviar(&conversa, "Segunda").await);

    let corpos = corpos_do_chat(&servidor);
    assert!(system_de(&corpos[0]).unwrap().contains("Gosta de café"));
    let segundo = system_de(&corpos[1]).unwrap();
    assert!(segundo.contains("Gosta de chá"));
    assert!(!segundo.contains("café"));
    // E as duas primeiras mensagens continuam idênticas depois da nova geração.
    assert_eq!(a.mensagens(&conversa)[..2], antes[..]);
}

#[tokio::test]
async fn uma_memoria_removida_nao_aparece_no_contexto_da_proxima_mensagem() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (projeto, conversa) = a.projeto("A", "Instruções.");
    let global = a.banco().criar_memoria_global("Segredo global").unwrap();
    let do_projeto = a
        .banco()
        .criar_memoria_de_projeto(&projeto, "Segredo do projeto")
        .unwrap();

    concluida(a.enviar(&conversa, "Um").await);
    let antes = a.mensagens(&conversa);
    a.banco().excluir_memoria(&global.id).unwrap();
    a.banco().excluir_memoria(&do_projeto.id).unwrap();
    // Excluir não mexe em nenhuma mensagem já gerada.
    assert_eq!(a.mensagens(&conversa), antes);
    concluida(a.enviar(&conversa, "Dois").await);

    let corpos = corpos_do_chat(&servidor);
    let primeiro = system_de(&corpos[0]).unwrap();
    assert!(primeiro.contains("Segredo global") && primeiro.contains("Segredo do projeto"));
    assert_eq!(
        system_de(&corpos[1]).as_deref(),
        Some("Instruções."),
        "sobram só as instruções"
    );
    assert!(!corpos[1].to_string().contains("Segredo"));
}

#[tokio::test]
async fn sem_instrucoes_e_sem_memorias_nao_vai_mensagem_system() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (_, conversa) = a.projeto("A", "");
    let memoria = a.banco().criar_memoria_global("Passageira").unwrap();
    a.banco().excluir_memoria(&memoria.id).unwrap();

    concluida(a.enviar(&conversa, "Oi").await);

    assert_eq!(system_de(&corpos_do_chat(&servidor)[0]), None);
}

// ----- Persistência -----

#[test]
fn as_memorias_continuam_depois_de_fechar_e_reabrir_o_banco() {
    let pasta = std::env::temp_dir().join(format!("memoteca-memorias-{}", novo_id()));
    fs::create_dir_all(&pasta).unwrap();
    let arquivo = pasta.join("memoteca.db");

    let projeto_id = {
        let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
        let projeto = banco
            .criar_projeto(&DadosProjeto {
                nome: "P".into(),
                system_prompt: String::new(),
                modelo_padrao: None,
            })
            .unwrap();
        banco.criar_memoria_global("Global que fica").unwrap();
        let editada = banco
            .criar_memoria_de_projeto(&projeto.id, "Rascunho")
            .unwrap();
        banco
            .atualizar_memoria(&editada.id, "De projeto, editada")
            .unwrap();
        projeto.id
    };

    let banco = Banco::abrir(&arquivo, &pasta.join("backups")).unwrap();
    let globais = banco.listar_memorias_globais().unwrap();
    let do_projeto = banco.listar_memorias_do_projeto(&projeto_id).unwrap();
    assert_eq!(globais.len(), 1);
    assert_eq!(globais[0].conteudo, "Global que fica");
    assert_eq!(do_projeto.len(), 1);
    assert_eq!(do_projeto[0].conteudo, "De projeto, editada");
    drop(banco);
    let _ = fs::remove_dir_all(&pasta);
}

// ----- Validação do texto -----

#[test]
fn memoria_em_branco_ou_grande_demais_e_recusada_com_mensagem_amigavel() {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "P".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let boa = banco
        .criar_memoria_global("  texto com espaços nas pontas  ")
        .unwrap();
    assert_eq!(boa.conteudo, "texto com espaços nas pontas");

    let grande = "a".repeat(MEMORIA_MAXIMA + 1);
    for texto in ["", "   \n\t", grande.as_str()] {
        for erro in [
            banco.criar_memoria_global(texto).unwrap_err(),
            banco
                .criar_memoria_de_projeto(&projeto.id, texto)
                .unwrap_err(),
            banco.atualizar_memoria(&boa.id, texto).unwrap_err(),
        ] {
            assert!(matches!(erro, ErroBanco::Rejeitado(_)), "{erro:?}");
            assert!(!erro.mensagem_amigavel().is_empty());
        }
    }
    // O limite exato é aceito, e a memória recusada não mexeu na anterior.
    banco
        .criar_memoria_global(&"a".repeat(MEMORIA_MAXIMA))
        .unwrap();
    assert_eq!(banco.obter_memoria(&boa.id).unwrap().conteudo, boa.conteudo);
    assert_eq!(banco.listar_memorias_globais().unwrap().len(), 2);
}

#[test]
fn editar_ou_excluir_uma_memoria_que_nao_existe_da_erro_sem_mexer_em_nada() {
    let banco = Banco::em_memoria().unwrap();
    banco.criar_memoria_global("Fica").unwrap();

    assert_eq!(
        banco.atualizar_memoria(&novo_id(), "x").unwrap_err(),
        ErroBanco::NaoEncontrado
    );
    assert_eq!(
        banco.excluir_memoria(&novo_id()).unwrap_err(),
        ErroBanco::NaoEncontrado
    );
    assert_eq!(banco.listar_memorias_globais().unwrap().len(), 1);
}

// ----- Aviso de contexto quase cheio -----

#[tokio::test]
async fn contexto_perto_do_limite_gera_aviso_sem_cortar_nada_e_sem_bloquear_o_envio() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (projeto, conversa) = a.projeto("A", "");
    // 600 caracteres de memória ≈ 204 tokens estimados, e o contexto é de 512.
    a.banco()
        .criar_memoria_de_projeto(&projeto, &"m".repeat(600))
        .unwrap();
    {
        let banco = a.banco();
        let atual = banco.obter_conversa(&conversa).unwrap();
        banco
            .atualizar_conversa(
                &conversa,
                &DadosConversa {
                    titulo: atual.titulo,
                    modelo: atual.modelo,
                    temperature: atual.temperature,
                    num_ctx: Some(512),
                    keep_alive: atual.keep_alive,
                },
            )
            .unwrap();
    }

    let aviso = concluida(a.enviar(&conversa, &"p".repeat(700)).await)
        .expect("deveria avisar: o contexto passou de 80% de 512");

    assert_eq!(aviso.limite, 512);
    assert!(aviso.estimado >= 410, "{aviso:?}");
    // Nada foi cortado: a memória e a pergunta inteira foram enviadas.
    let corpo = &corpos_do_chat(&servidor)[0];
    let mensagens = corpo["messages"].as_array().unwrap();
    assert!(
        mensagens[0]["content"]
            .as_str()
            .unwrap()
            .contains(&"m".repeat(600))
    );
    assert_eq!(mensagens[1]["content"].as_str().unwrap().len(), 700);
    assert_eq!(corpo["options"]["num_ctx"], 512);
}

#[tokio::test]
async fn contexto_folgado_nao_gera_aviso() {
    let servidor = servidor().await;
    let a = Ambiente::novo(&servidor.url());
    let (_, conversa) = a.projeto("A", "Curto.");

    assert_eq!(concluida(a.enviar(&conversa, "Oi").await), None);
}
