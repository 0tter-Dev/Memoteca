//! Servidor Ollama falso para os testes (E05).
//!
//! Imita, em HTTP/1.1 puro sobre `tokio`, as respostas do Ollama que o Memoteca usa e
//! permite **injetar falhas**: linha partida em dois pedaços, JSON inválido, queda no meio
//! do stream, silêncio, resposta de outro programa, conexão aceita e fechada, etc.
//!
//! O arquivo é incluído por `#[path]` nos testes de integração, e é a implementação única
//! que o E2E da E15 também vai usar. Nada daqui entra no app.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Um passo de uma resposta em stream (NDJSON em `Transfer-Encoding: chunked`).
#[derive(Clone)]
pub enum Passo {
    /// Envia a linha, mais `\n`, como um pedaço HTTP.
    Linha(String),
    /// Envia estes bytes, como estão, num único pedaço HTTP. Serve para partir linhas.
    Bytes(Vec<u8>),
    /// Espera antes do próximo passo.
    Pausa(Duration),
    /// Repete a linha até o cliente desconectar.
    LinhaRepetida { linha: String, intervalo: Duration },
    /// Não envia mais nada e espera o cliente desistir.
    Silencio,
    /// Fecha a conexão no meio, sem o pedaço final do HTTP.
    Cortar,
    /// Termina a resposta normalmente (pedaço final do HTTP).
    Terminar,
}

/// O que o servidor responde a uma rota.
#[derive(Clone)]
pub enum Resposta {
    Json {
        status: u16,
        corpo: String,
    },
    Stream(Vec<Passo>),
    /// Aceita a conexão e fecha sem responder.
    AceitarEFechar,
    /// Responde bytes que nem são HTTP.
    NaoEHttp(Vec<u8>),
    /// Lê o pedido e nunca responde.
    NaoResponder,
}

impl Resposta {
    pub fn json(corpo: &str) -> Self {
        Self::Json {
            status: 200,
            corpo: corpo.to_owned(),
        }
    }

    pub fn erro(status: u16, mensagem: &str) -> Self {
        Self::Json {
            status,
            corpo: format!("{{\"error\":\"{mensagem}\"}}"),
        }
    }

    /// Stream que termina normalmente depois das linhas.
    pub fn linhas(linhas: &[&str]) -> Self {
        let mut passos: Vec<Passo> = linhas
            .iter()
            .map(|l| Passo::Linha((*l).to_owned()))
            .collect();
        passos.push(Passo::Terminar);
        Self::Stream(passos)
    }
}

#[derive(Debug, Clone)]
pub struct Requisicao {
    pub metodo: String,
    pub caminho: String,
    pub corpo: String,
}

struct Estado {
    rotas: Mutex<HashMap<String, Resposta>>,
    requisicoes: Mutex<Vec<Requisicao>>,
    /// Quantas vezes o cliente fechou a conexão antes de o servidor terminar de responder.
    desconexoes: AtomicUsize,
}

pub struct OllamaFalso {
    endereco: SocketAddr,
    estado: Arc<Estado>,
    tarefa: JoinHandle<()>,
}

impl OllamaFalso {
    /// Sobe o servidor numa porta livre de `127.0.0.1`.
    pub async fn iniciar() -> Self {
        Self::iniciar_em("127.0.0.1:0").await
    }

    /// Sobe o servidor num endereço específico (por exemplo, o de um teste que simula o
    /// Ollama "subindo" depois de ser iniciado).
    pub async fn iniciar_em(endereco: &str) -> Self {
        let ouvinte = TcpListener::bind(endereco)
            .await
            .expect("abrir a porta local pedida");
        let endereco = ouvinte.local_addr().unwrap();
        let estado = Arc::new(Estado {
            rotas: Mutex::new(HashMap::new()),
            requisicoes: Mutex::new(Vec::new()),
            desconexoes: AtomicUsize::new(0),
        });

        let estado_da_tarefa = Arc::clone(&estado);
        let tarefa = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = ouvinte.accept().await else {
                    break;
                };
                tokio::spawn(atender(socket, Arc::clone(&estado_da_tarefa)));
            }
        });

        Self {
            endereco,
            estado,
            tarefa,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.endereco)
    }

    /// Define a resposta de uma rota, por exemplo `("GET", "/api/version")`.
    pub fn rota(&self, metodo: &str, caminho: &str, resposta: Resposta) {
        self.estado
            .rotas
            .lock()
            .unwrap()
            .insert(format!("{metodo} {caminho}"), resposta);
    }

    /// Pedidos recebidos até agora, na ordem.
    pub fn requisicoes(&self) -> Vec<Requisicao> {
        self.estado.requisicoes.lock().unwrap().clone()
    }

    /// Espera o cliente fechar a conexão no meio de uma resposta. `true` se aconteceu.
    pub async fn esperar_desconexao_do_cliente(&self, limite: Duration) -> bool {
        let inicio = tokio::time::Instant::now();
        while inicio.elapsed() < limite {
            if self.estado.desconexoes.load(Ordering::SeqCst) > 0 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }
}

impl Drop for OllamaFalso {
    fn drop(&mut self) {
        self.tarefa.abort();
    }
}

/// Endereço local onde ninguém está ouvindo: conexão recusada.
pub async fn endereco_sem_ninguem() -> String {
    format!("http://{}", porta_livre().await)
}

/// Endereço `ip:porta` de uma porta local que acabou de ser liberada.
pub async fn porta_livre() -> SocketAddr {
    let ouvinte = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endereco = ouvinte.local_addr().unwrap();
    drop(ouvinte);
    endereco
}

// ----- Atendimento -----

async fn atender(mut socket: TcpStream, estado: Arc<Estado>) {
    let Some(requisicao) = ler_requisicao(&mut socket).await else {
        return;
    };
    estado.requisicoes.lock().unwrap().push(requisicao.clone());

    let resposta = estado
        .rotas
        .lock()
        .unwrap()
        .get(&format!("{} {}", requisicao.metodo, requisicao.caminho))
        .cloned();

    match resposta {
        None => {
            responder_json(
                socket,
                404,
                "{\"error\":\"rota não configurada no servidor falso\"}",
            )
            .await
        }
        Some(Resposta::Json { status, corpo }) => responder_json(socket, status, &corpo).await,
        Some(Resposta::AceitarEFechar) => drop(socket),
        Some(Resposta::NaoEHttp(bytes)) => {
            let _ = socket.write_all(&bytes).await;
            let _ = socket.shutdown().await;
        }
        Some(Resposta::NaoResponder) => esperar_cliente_fechar(socket, &estado).await,
        Some(Resposta::Stream(passos)) => responder_stream(socket, passos, estado).await,
    }
}

async fn ler_requisicao(socket: &mut TcpStream) -> Option<Requisicao> {
    let mut buffer = Vec::new();
    let mut pedaco = [0u8; 4096];

    let fim_do_cabecalho = loop {
        if let Some(posicao) = buffer.windows(4).position(|janela| janela == b"\r\n\r\n") {
            break posicao;
        }
        let lidos = socket.read(&mut pedaco).await.ok()?;
        if lidos == 0 {
            return None;
        }
        buffer.extend_from_slice(&pedaco[..lidos]);
    };

    let cabecalho = String::from_utf8_lossy(&buffer[..fim_do_cabecalho]).into_owned();
    let mut linhas = cabecalho.lines();
    let mut primeira = linhas.next()?.split_whitespace();
    let metodo = primeira.next()?.to_owned();
    let caminho = primeira.next()?.to_owned();
    let tamanho: usize = linhas
        .filter_map(|linha| {
            let (nome, valor) = linha.split_once(':')?;
            nome.eq_ignore_ascii_case("content-length")
                .then(|| valor.trim().parse().ok())
                .flatten()
        })
        .next()
        .unwrap_or(0);

    let inicio_do_corpo = fim_do_cabecalho + 4;
    while buffer.len() < inicio_do_corpo + tamanho {
        let lidos = socket.read(&mut pedaco).await.ok()?;
        if lidos == 0 {
            return None;
        }
        buffer.extend_from_slice(&pedaco[..lidos]);
    }
    let corpo =
        String::from_utf8_lossy(&buffer[inicio_do_corpo..inicio_do_corpo + tamanho]).into_owned();

    Some(Requisicao {
        metodo,
        caminho,
        corpo,
    })
}

async fn responder_json(mut socket: TcpStream, status: u16, corpo: &str) {
    let resposta = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        motivo_http(status),
        corpo.len()
    );
    let _ = socket.write_all(resposta.as_bytes()).await;
    let _ = socket.shutdown().await;
}

fn motivo_http(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

async fn esperar_cliente_fechar(mut socket: TcpStream, estado: &Estado) {
    let mut lixo = [0u8; 64];
    loop {
        match socket.read(&mut lixo).await {
            Ok(0) | Err(_) => {
                estado.desconexoes.fetch_add(1, Ordering::SeqCst);
                return;
            }
            Ok(_) => {}
        }
    }
}

async fn responder_stream(socket: TcpStream, passos: Vec<Passo>, estado: Arc<Estado>) {
    let (mut leitor, mut escritor) = socket.into_split();

    // Observa o lado de leitura: se o cliente fechar antes de terminarmos, é desconexão.
    let (avisar, mut desconectou) = watch::channel(false);
    let concluido = Arc::new(AtomicBool::new(false));
    let concluido_do_observador = Arc::clone(&concluido);
    let estado_do_observador = Arc::clone(&estado);
    let observador = tokio::spawn(async move {
        let mut lixo = [0u8; 64];
        loop {
            match leitor.read(&mut lixo).await {
                Ok(0) | Err(_) => {
                    if !concluido_do_observador.load(Ordering::SeqCst) {
                        estado_do_observador
                            .desconexoes
                            .fetch_add(1, Ordering::SeqCst);
                    }
                    let _ = avisar.send(true);
                    return;
                }
                Ok(_) => {}
            }
        }
    });

    let cabecalho = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
    if escritor.write_all(cabecalho.as_bytes()).await.is_ok() {
        executar_passos(&mut escritor, passos, &mut desconectou).await;
    }

    concluido.store(true, Ordering::SeqCst);
    observador.abort();
}

async fn executar_passos(
    escritor: &mut OwnedWriteHalf,
    passos: Vec<Passo>,
    desconectou: &mut watch::Receiver<bool>,
) {
    for passo in passos {
        match passo {
            Passo::Linha(linha) => {
                if !escrever_pedaco(escritor, format!("{linha}\n").as_bytes()).await {
                    return;
                }
            }
            Passo::Bytes(bytes) => {
                if !escrever_pedaco(escritor, &bytes).await {
                    return;
                }
            }
            Passo::Pausa(duracao) => {
                if !dormir(duracao, desconectou).await {
                    return;
                }
            }
            Passo::LinhaRepetida { linha, intervalo } => loop {
                if !escrever_pedaco(escritor, format!("{linha}\n").as_bytes()).await {
                    return;
                }
                if !dormir(intervalo, desconectou).await {
                    return;
                }
            },
            Passo::Silencio => {
                let _ = desconectou.wait_for(|fechou| *fechou).await;
                return;
            }
            Passo::Cortar => return,
            Passo::Terminar => {
                let _ = escritor.write_all(b"0\r\n\r\n").await;
                let _ = escritor.flush().await;
            }
        }
    }
}

async fn escrever_pedaco(escritor: &mut OwnedWriteHalf, dados: &[u8]) -> bool {
    let mut pedaco = format!("{:x}\r\n", dados.len()).into_bytes();
    pedaco.extend_from_slice(dados);
    pedaco.extend_from_slice(b"\r\n");
    escritor.write_all(&pedaco).await.is_ok() && escritor.flush().await.is_ok()
}

/// Espera `duracao`. Devolve `false` se o cliente desconectou nesse meio tempo.
async fn dormir(duracao: Duration, desconectou: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        () = tokio::time::sleep(duracao) => true,
        _ = desconectou.wait_for(|fechou| *fechou) => false,
    }
}
