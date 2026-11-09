//! Testes de aceite da E08: download guiado de modelos contra o servidor Ollama falso.
//! Os testes dos cenários do plano de robustez citam o identificador `Rnn` no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use memoteca_lib::instalador::disco::EspacoEmDisco;
use memoteca_lib::modelos::{
    ErroModelo, FaseDoDownload, ProgressoDoModelo, ResultadoDoModelo, ServicoDeModelos,
    catalogo_embutido,
};
use memoteca_lib::ollama::{ClienteOllama, ErroOllama, Limites};
use ollama_falso::{OllamaFalso, Passo, Resposta, endereco_sem_ninguem};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

const GB: u64 = 1024 * 1024 * 1024;

/// Espaço livre que o teste controla. Cada leitura consome o próximo valor da fila, e o último
/// se repete. `None` simula um disco que não pode ser lido.
struct EspacoFalso(Mutex<Vec<Option<u64>>>);

impl EspacoFalso {
    fn fixo(livre: u64) -> Arc<Self> {
        Arc::new(Self(Mutex::new(vec![Some(livre)])))
    }
}

impl EspacoEmDisco for EspacoFalso {
    fn livre_em(&self, _pasta: &Path) -> io::Result<u64> {
        let mut fila = self.0.lock().unwrap();
        let valor = if fila.len() > 1 {
            fila.remove(0)
        } else {
            fila[0]
        };
        valor.ok_or_else(|| io::Error::other("não foi possível ler o espaço"))
    }
}

fn limites() -> Limites {
    Limites {
        conexao: Duration::from_secs(4),
        consulta: Duration::from_secs(6),
        ociosidade_chat: Duration::from_millis(600),
        ociosidade_pull: Duration::from_millis(600),
    }
}

fn servico(url: &str, espaco: Arc<EspacoFalso>) -> ServicoDeModelos {
    ServicoDeModelos::novo(
        ClienteOllama::com_endereco(url, limites()).unwrap(),
        espaco,
        Some(PathBuf::from(r"E:\modelos")),
        catalogo_embutido().unwrap(),
    )
}

fn linha_pull(digest: &str, total: u64, baixado: u64) -> String {
    format!(
        r#"{{"status":"pulling {digest}","digest":"{digest}","total":{total},"completed":{baixado}}}"#
    )
}

fn pull_completo(total: u64) -> Resposta {
    Resposta::linhas(&[
        r#"{"status":"pulling manifest"}"#,
        &linha_pull("aaa", total, 0),
        &linha_pull("aaa", total, total / 2),
        &linha_pull("aaa", total, total),
        r#"{"status":"verifying sha256 digest"}"#,
        r#"{"status":"writing manifest"}"#,
        r#"{"status":"success"}"#,
    ])
}

fn tags(nomes: &[&str]) -> Resposta {
    let modelos: Vec<String> = nomes
        .iter()
        .map(|n| format!(r#"{{"name":"{n}","size":1}}"#))
        .collect();
    Resposta::json(&format!(r#"{{"models":[{}]}}"#, modelos.join(",")))
}

async fn servidor(pull: Resposta, instalados: &[&str]) -> OllamaFalso {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota("POST", "/api/pull", pull);
    servidor.rota("GET", "/api/tags", tags(instalados));
    servidor
}

async fn baixar(
    servico: &ServicoDeModelos,
    nome: &str,
) -> (
    Result<ResultadoDoModelo, ErroModelo>,
    Vec<ProgressoDoModelo>,
) {
    let mut progressos = Vec::new();
    let resultado = servico
        .baixar(nome, &CancellationToken::new(), &mut |p| progressos.push(p))
        .await;
    (resultado, progressos)
}

fn pedidos_de_pull(servidor: &OllamaFalso) -> usize {
    servidor
        .requisicoes()
        .iter()
        .filter(|r| r.caminho == "/api/pull")
        .count()
}

// ----- Caminho feliz -----

#[tokio::test]
async fn baixa_um_modelo_do_catalogo_com_progresso_e_confere_que_ele_apareceu() {
    let servidor = servidor(pull_completo(2_019_393_189), &["llama3.2:3b"]).await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, progressos) = baixar(&servico, "llama3.2:3b").await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Concluido));
    let fases: Vec<FaseDoDownload> = progressos.iter().map(|p| p.fase).collect();
    assert_eq!(fases.first(), Some(&FaseDoDownload::Preparando));
    assert!(fases.contains(&FaseDoDownload::Baixando));
    assert!(fases.contains(&FaseDoDownload::Verificando));
    assert!(fases.contains(&FaseDoDownload::Finalizando));
    assert_eq!(fases.last(), Some(&FaseDoDownload::Concluido));
    let ultimo_baixando = progressos
        .iter()
        .rfind(|p| p.fase == FaseDoDownload::Baixando)
        .unwrap();
    assert_eq!(
        ultimo_baixando.baixado_em_bytes,
        ultimo_baixando.total_em_bytes
    );

    let enviado = servidor
        .requisicoes()
        .into_iter()
        .find(|r| r.caminho == "/api/pull")
        .unwrap();
    let corpo: Value = serde_json::from_str(&enviado.corpo).unwrap();
    assert_eq!(corpo["model"], "llama3.2:3b");
}

#[tokio::test]
async fn um_nome_sem_tag_e_encontrado_na_lista_como_latest() {
    let servidor = servidor(pull_completo(1000), &["meu-modelo:latest"]).await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, _) = baixar(&servico, "meu-modelo").await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Concluido));
}

#[tokio::test]
async fn download_que_termina_mas_nao_aparece_na_lista_e_erro_amigavel() {
    let servidor = servidor(pull_completo(1000), &["outro:1b"]).await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, _) = baixar(&servico, "llama3.2:3b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroModelo::NaoApareceuNaLista);
    assert!(erro.mensagem_amigavel().contains("não apareceu"));
}

// ----- Espaço em disco (R07) -----

#[tokio::test]
async fn r07_sem_espaco_um_modelo_do_catalogo_e_bloqueado_antes_de_comecar() {
    let servidor = servidor(pull_completo(2_019_393_189), &["llama3.2:3b"]).await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(GB)); // o modelo tem ~2 GB

    let (resultado, progressos) = baixar(&servico, "llama3.2:3b").await;

    let erro = resultado.unwrap_err();
    let ErroModelo::SemEspaco {
        necessario_em_bytes,
        disponivel_em_bytes,
    } = erro
    else {
        panic!("deveria faltar espaço: {erro:?}");
    };
    assert_eq!(necessario_em_bytes, 2_019_393_189);
    assert_eq!(disponivel_em_bytes, GB);
    assert!(erro.mensagem_amigavel().contains("espaço livre"));
    assert_eq!(
        pedidos_de_pull(&servidor),
        0,
        "não pode nem começar o download"
    );
    assert!(progressos.is_empty());
}

#[tokio::test]
async fn com_pouca_folga_o_download_segue_porque_a_tela_ja_alertou() {
    let servidor = servidor(pull_completo(2_019_393_189), &["llama3.2:3b"]).await;
    // Cabe, mas sobra 1 GB (menos que os 5 GB de folga): é alerta, não bloqueio.
    let servico = servico(&servidor.url(), EspacoFalso::fixo(2_019_393_189 + GB));

    let (avaliacao, _, _) = servico.avaliar_espaco("llama3.2:3b");
    assert!(matches!(
        avaliacao,
        memoteca_lib::modelos::AvaliacaoDeEspaco::Alerta { .. }
    ));
    let (resultado, _) = baixar(&servico, "llama3.2:3b").await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Concluido));
}

#[tokio::test]
async fn r07_modelo_digitado_a_mao_que_nao_cabe_e_interrompido_assim_que_o_tamanho_aparece() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![
            Passo::Linha(r#"{"status":"pulling manifest"}"#.into()),
            Passo::Linha(linha_pull("bbb", 20 * GB, 0)),
            Passo::Silencio,
        ]),
    );
    // Fora do catálogo: o tamanho só se conhece depois que o download começa.
    let servico = servico(&servidor.url(), EspacoFalso::fixo(2 * GB));

    let (avaliacao, _, _) = servico.avaliar_espaco("alguem/modelo-grande:70b");
    assert_eq!(
        avaliacao,
        memoteca_lib::modelos::AvaliacaoDeEspaco::Desconhecido
    );
    let (resultado, _) = baixar(&servico, "alguem/modelo-grande:70b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(
        erro,
        ErroModelo::SemEspaco {
            necessario_em_bytes: 20 * GB,
            disponivel_em_bytes: 2 * GB
        }
    );
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await,
        "o download deve ser interrompido"
    );
}

#[tokio::test]
async fn o_espaco_conta_so_o_que_falta_baixar_ao_retomar() {
    // O modelo tem 20 GB, mas 19 GB já estavam baixados de uma tentativa anterior.
    let servidor = servidor(
        Resposta::linhas(&[
            &linha_pull("ccc", 20 * GB, 19 * GB),
            &linha_pull("ccc", 20 * GB, 20 * GB),
            r#"{"status":"success"}"#,
        ]),
        &["alguem/retomado:latest"],
    )
    .await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(3 * GB));

    let (resultado, _) = baixar(&servico, "alguem/retomado").await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Concluido));
}

#[tokio::test]
async fn se_o_espaco_nao_puder_ser_lido_o_download_nao_e_bloqueado() {
    let servidor = servidor(pull_completo(2_019_393_189), &["llama3.2:3b"]).await;
    let servico = servico(
        &servidor.url(),
        Arc::new(EspacoFalso(Mutex::new(vec![None]))),
    );

    let (resultado, _) = baixar(&servico, "llama3.2:3b").await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Concluido));
}

// ----- Download interrompido (R09) -----

#[tokio::test]
async fn r09_download_interrompido_da_mensagem_amigavel_e_pode_ser_continuado() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![
            Passo::Linha(r#"{"status":"pulling manifest"}"#.into()),
            Passo::Linha(linha_pull("aaa", 1000, 400)),
            Passo::Cortar,
        ]),
    );
    servidor.rota("GET", "/api/tags", tags(&["llama3.2:3b"]));
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, progressos) = baixar(&servico, "llama3.2:3b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroModelo::Ollama(ErroOllama::ConexaoInterrompida));
    assert!(erro.pode_continuar());
    let mensagem = erro.mensagem_amigavel();
    assert!(mensagem.contains("guardado") && mensagem.contains("Continuar"));
    assert_eq!(progressos.last().unwrap().baixado_em_bytes, 400);

    // Estado limpo: nada ficou como modelo instalado, e continuar usa a mesma chamada, que o
    // Ollama retoma do ponto onde parou.
    servidor.rota("POST", "/api/pull", pull_completo(1000));
    let (segunda, _) = baixar(&servico, "llama3.2:3b").await;
    assert_eq!(segunda, Ok(ResultadoDoModelo::Concluido));
    assert_eq!(pedidos_de_pull(&servidor), 2);
}

#[tokio::test]
async fn download_travado_sem_dados_tambem_oferece_continuar() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![
            Passo::Linha(r#"{"status":"pulling manifest"}"#.into()),
            Passo::Silencio,
        ]),
    );
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, _) = baixar(&servico, "llama3.2:3b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroModelo::Ollama(ErroOllama::TempoEsgotado));
    assert!(erro.pode_continuar());
}

// ----- Modelo inexistente (R10) e nome inválido -----

#[tokio::test]
async fn r10_modelo_inexistente_da_mensagem_amigavel_e_nao_o_erro_cru_da_api() {
    let servidor = servidor(
        Resposta::linhas(&[
            r#"{"status":"pulling manifest"}"#,
            r#"{"error":"pull model manifest: file does not exist"}"#,
        ]),
        &[],
    )
    .await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    let (resultado, _) = baixar(&servico, "modelo-que-nao-existe:9b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroModelo::Ollama(ErroOllama::ModeloInexistente));
    let mensagem = erro.mensagem_amigavel();
    assert!(mensagem.contains("Confira o nome"));
    assert!(!mensagem.contains("manifest") && !mensagem.contains("file does not exist"));
    assert!(!erro.pode_continuar());
}

#[tokio::test]
async fn nome_invalido_e_recusado_sem_falar_com_o_ollama() {
    let servidor = servidor(pull_completo(1000), &[]).await;
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));

    for nome in ["", "tem espaço", "http://exemplo.com/m", "../x", "a;b"] {
        let (resultado, _) = baixar(&servico, nome).await;

        let erro = resultado.unwrap_err();
        assert!(matches!(erro, ErroModelo::Nome(_)), "{nome:?}");
        assert!(erro.mensagem_amigavel().contains("llama3.2:3b"), "{nome:?}");
    }
    assert!(servidor.requisicoes().is_empty());
}

// ----- Falhas de conexão e cancelamento -----

#[tokio::test]
async fn sem_ollama_respondendo_o_erro_e_amigavel() {
    let url = endereco_sem_ninguem().await;
    let servico = ServicoDeModelos::novo(
        ClienteOllama::com_endereco(
            &url,
            Limites {
                conexao: Duration::from_secs(4),
                consulta: Duration::from_secs(6),
                ociosidade_chat: Duration::from_secs(6),
                ociosidade_pull: Duration::from_secs(6),
            },
        )
        .unwrap(),
        EspacoFalso::fixo(100 * GB),
        None,
        catalogo_embutido().unwrap(),
    );

    let (resultado, _) = baixar(&servico, "llama3.2:3b").await;

    let erro = resultado.unwrap_err();
    assert_eq!(erro, ErroModelo::Ollama(ErroOllama::OllamaForaDoAr));
    assert!(erro.mensagem_amigavel().contains("Ollama"));
}

#[tokio::test]
async fn cancelar_o_download_fecha_a_conexao_e_nao_e_erro() {
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "POST",
        "/api/pull",
        Resposta::Stream(vec![Passo::LinhaRepetida {
            linha: linha_pull("aaa", 1_000_000, 10),
            intervalo: Duration::from_millis(5),
        }]),
    );
    let servico = servico(&servidor.url(), EspacoFalso::fixo(100 * GB));
    let cancelamento = CancellationToken::new();
    let mut recebidos = 0;

    let resultado = servico
        .baixar("llama3.2:3b", &cancelamento, &mut |_| {
            recebidos += 1;
            if recebidos == 3 {
                cancelamento.cancel();
            }
        })
        .await;

    assert_eq!(resultado, Ok(ResultadoDoModelo::Cancelado));
    assert!(
        servidor
            .esperar_desconexao_do_cliente(Duration::from_secs(3))
            .await
    );
}
