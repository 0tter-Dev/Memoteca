pub mod banco;
pub mod instalador;
pub mod modelos;
pub mod ollama;

use std::sync::{Arc, Mutex};

use banco::Banco;
use instalador::{InstaladorDoOllama, ProgressoInstalacao, ResultadoInstalacao};
use modelos::{
    AvaliacaoParaInterface, Catalogo, ProgressoDoModelo, ResultadoDoModelo, ServicoDeModelos,
};
use ollama::ClienteOllama;
use ollama::ciclo::{EstadoParaInterface, GerenciadorOllama};
use ollama::instalacao::DetectorReal;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, RunEvent};
use tokio_util::sync::CancellationToken;

/// Resultado de abrir o banco na inicialização. Em caso de falha guarda a mensagem
/// amigável, que a interface mostra ao usuário.
struct EstadoBanco(Result<Mutex<Banco>, String>);

/// A interface chama este comando ao abrir. `Ok` significa que os dados locais estão
/// prontos, e `Err` traz a mensagem amigável em pt-BR.
#[tauri::command]
fn estado_do_banco(estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    estado.0.as_ref().map(|_| ()).map_err(Clone::clone)
}

/// Gerenciador do Ollama, compartilhado pelos comandos. O `Mutex` é assíncrono porque
/// `preparar` fica esperando o servidor subir. Em caso de falha ao criar o cliente guarda a
/// mensagem amigável.
struct OllamaDoApp(Result<tokio::sync::Mutex<GerenciadorOllama>, String>);

/// A interface chama este comando ao abrir: detecta o Ollama, reutiliza o que já está
/// rodando ou inicia o instalado, e devolve o estado em formato simples para a tela.
#[tauri::command]
async fn preparar_ollama(
    estado: tauri::State<'_, OllamaDoApp>,
) -> Result<EstadoParaInterface, String> {
    let gerenciador = estado.0.as_ref().map_err(Clone::clone)?;
    let mut gerenciador = gerenciador.lock().await;
    Ok(gerenciador.preparar().await.para_interface())
}

/// Instalador do Ollama e o cancelamento da instalação em andamento, se houver. Só uma
/// instalação roda por vez.
struct InstalacaoDoApp {
    instalador: Result<InstaladorDoOllama, String>,
    em_andamento: Mutex<Option<CancellationToken>>,
}

/// Libera a vaga de "instalação em andamento" ao sair, em qualquer desfecho.
struct VagaDeInstalacao<'a>(&'a Mutex<Option<CancellationToken>>);

impl Drop for VagaDeInstalacao<'_> {
    fn drop(&mut self) {
        if let Ok(mut vaga) = self.0.lock() {
            *vaga = None;
        }
    }
}

/// Baixa, valida e instala o Ollama, enviando o progresso pelo `canal`. Devolve `concluida`
/// ou `cancelada`, ou a mensagem amigável do que deu errado.
#[tauri::command]
async fn instalar_ollama(
    canal: Channel<ProgressoInstalacao>,
    estado: tauri::State<'_, InstalacaoDoApp>,
) -> Result<&'static str, String> {
    let instalador = estado.instalador.as_ref().map_err(Clone::clone)?;

    let cancelamento = CancellationToken::new();
    {
        let mut vaga = estado
            .em_andamento
            .lock()
            .map_err(|_| "Tente de novo.".to_owned())?;
        if vaga.is_some() {
            return Err("Já há uma instalação do Ollama em andamento.".to_owned());
        }
        *vaga = Some(cancelamento.clone());
    }
    let _vaga = VagaDeInstalacao(&estado.em_andamento);

    let resultado = instalador
        .instalar(&cancelamento, &mut |progresso| {
            let _ = canal.send(progresso);
        })
        .await;
    match resultado {
        Ok(ResultadoInstalacao::Concluida { .. }) => Ok("concluida"),
        Ok(ResultadoInstalacao::Cancelada) => Ok("cancelada"),
        Err(falha) => {
            eprintln!("a instalação do Ollama falhou: {falha}");
            Err(falha.mensagem_amigavel())
        }
    }
}

/// Cancela a instalação em andamento. Não faz nada se não houver nenhuma.
#[tauri::command]
fn cancelar_instalacao_ollama(estado: tauri::State<'_, InstalacaoDoApp>) {
    if let Ok(vaga) = estado.em_andamento.lock()
        && let Some(cancelamento) = vaga.as_ref()
    {
        cancelamento.cancel();
    }
}

/// Chave de configuração que marca o onboarding como concluído.
const CONFIG_ONBOARDING_CONCLUIDO: &str = "onboarding_concluido";

/// O usuário já concluiu o onboarding?
#[tauri::command]
fn onboarding_concluido(estado: tauri::State<'_, EstadoBanco>) -> Result<bool, String> {
    let banco = estado.0.as_ref().map_err(Clone::clone)?;
    let banco = banco.lock().map_err(|_| "Tente de novo.".to_owned())?;
    banco
        .obter_configuracao(CONFIG_ONBOARDING_CONCLUIDO)
        .map(|valor| valor.as_deref() == Some("sim"))
        .map_err(|erro| erro.mensagem_amigavel())
}

/// Marca o onboarding como concluído.
#[tauri::command]
fn concluir_onboarding(estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    let banco = estado.0.as_ref().map_err(Clone::clone)?;
    let banco = banco.lock().map_err(|_| "Tente de novo.".to_owned())?;
    banco
        .definir_configuracao(CONFIG_ONBOARDING_CONCLUIDO, "sim")
        .map_err(|erro| erro.mensagem_amigavel())
}

/// Catálogo de modelos, o espaço em disco e o download em andamento, se houver. Só um
/// download roda por vez.
struct ModelosDoApp {
    servico: Result<ServicoDeModelos, String>,
    em_andamento: Mutex<Option<CancellationToken>>,
}

/// Falha do download, no formato que a tela recebe.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FalhaDoDownloadParaInterface {
    mensagem: String,
    /// Tentar de novo retoma do ponto onde parou (o botão diz "Continuar").
    pode_continuar: bool,
}

impl FalhaDoDownloadParaInterface {
    fn simples(mensagem: &str) -> Self {
        Self {
            mensagem: mensagem.to_owned(),
            pode_continuar: false,
        }
    }
}

/// O catálogo curado de modelos, já validado.
#[tauri::command]
fn catalogo_de_modelos(estado: tauri::State<'_, ModelosDoApp>) -> Result<Catalogo, String> {
    let servico = estado.servico.as_ref().map_err(Clone::clone)?;
    Ok(servico.catalogo().clone())
}

/// Avalia o espaço em disco para um modelo do catálogo, antes de baixar.
#[tauri::command]
fn avaliar_espaco_do_modelo(
    tag: String,
    estado: tauri::State<'_, ModelosDoApp>,
) -> Result<AvaliacaoParaInterface, String> {
    let servico = estado.servico.as_ref().map_err(Clone::clone)?;
    let (avaliacao, necessario, livre) = servico.avaliar_espaco(&tag);
    Ok(avaliacao.para_interface(necessario, livre))
}

/// Baixa um modelo, enviando o progresso pelo `canal`. Devolve `concluido` ou `cancelado`,
/// ou a falha com a mensagem amigável.
#[tauri::command]
async fn baixar_modelo(
    nome: String,
    canal: Channel<ProgressoDoModelo>,
    estado: tauri::State<'_, ModelosDoApp>,
    ollama_do_app: tauri::State<'_, OllamaDoApp>,
) -> Result<&'static str, FalhaDoDownloadParaInterface> {
    let servico = estado
        .servico
        .as_ref()
        .map_err(|mensagem| FalhaDoDownloadParaInterface::simples(mensagem))?;

    // Garante que o Ollama está no ar. Se ele caiu no meio de um download, é aqui que o
    // "Continuar" o inicia de novo, e o download retoma de onde parou.
    let gerenciador = ollama_do_app
        .0
        .as_ref()
        .map_err(|mensagem| FalhaDoDownloadParaInterface::simples(mensagem))?;
    let situacao = gerenciador.lock().await.preparar().await;
    if !matches!(situacao, ollama::ciclo::EstadoOllama::EmExecucao(_)) {
        let mensagem = situacao
            .para_interface()
            .mensagem
            .unwrap_or_else(|| "O Ollama não está disponível. Tente de novo.".to_owned());
        return Err(FalhaDoDownloadParaInterface::simples(&mensagem));
    }

    let cancelamento = CancellationToken::new();
    {
        let mut vaga = estado
            .em_andamento
            .lock()
            .map_err(|_| FalhaDoDownloadParaInterface::simples("Tente de novo."))?;
        if vaga.is_some() {
            return Err(FalhaDoDownloadParaInterface::simples(
                "Já há um download de modelo em andamento.",
            ));
        }
        *vaga = Some(cancelamento.clone());
    }
    let _vaga = VagaDeInstalacao(&estado.em_andamento);

    let resultado = servico
        .baixar(&nome, &cancelamento, &mut |progresso| {
            let _ = canal.send(progresso);
        })
        .await;
    match resultado {
        Ok(ResultadoDoModelo::Concluido) => Ok("concluido"),
        Ok(ResultadoDoModelo::Cancelado) => Ok("cancelado"),
        Err(erro) => {
            eprintln!("o download do modelo falhou: {erro}");
            Err(FalhaDoDownloadParaInterface {
                mensagem: erro.mensagem_amigavel(),
                pode_continuar: erro.pode_continuar(),
            })
        }
    }
}

/// Cancela o download de modelo em andamento. Não faz nada se não houver nenhum.
#[tauri::command]
fn cancelar_download_modelo(estado: tauri::State<'_, ModelosDoApp>) {
    if let Ok(vaga) = estado.em_andamento.lock()
        && let Some(cancelamento) = vaga.as_ref()
    {
        cancelamento.cancel();
    }
}

/// Abre o banco na pasta de dados do app. O texto técnico do erro vai para o log de erro
/// padrão, e só a mensagem amigável sobe para a interface.
fn abrir_banco(app: &tauri::AppHandle) -> Result<Mutex<Banco>, String> {
    let pasta = app.path().app_data_dir().map_err(|erro| {
        eprintln!("não foi possível localizar a pasta de dados: {erro}");
        banco::ErroBanco::Indisponivel(erro.to_string()).mensagem_amigavel()
    })?;

    Banco::abrir(&pasta.join("memoteca.db"), &pasta.join("backups"))
        .map(Mutex::new)
        .map_err(|erro| {
            eprintln!("não foi possível abrir o banco de dados: {erro}");
            erro.mensagem_amigavel()
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // Janela única: uma segunda execução apenas traz a janela existente para a frente.
    // O plugin precisa ser o primeiro a ser registrado.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
        .setup(|app| {
            let estado = EstadoBanco(abrir_banco(app.handle()));
            app.manage(estado);

            let ollama = GerenciadorOllama::novo()
                .map(tokio::sync::Mutex::new)
                .map_err(|erro| {
                    eprintln!("não foi possível preparar o gerenciador do Ollama: {erro}");
                    erro.mensagem_amigavel()
                });
            app.manage(OllamaDoApp(ollama));

            let instalador = InstaladorDoOllama::novo(Arc::new(DetectorReal)).map_err(|erro| {
                eprintln!("não foi possível preparar o instalador do Ollama: {erro}");
                erro.mensagem_amigavel()
            });
            app.manage(InstalacaoDoApp {
                instalador,
                em_andamento: Mutex::new(None),
            });

            let servico = ClienteOllama::novo()
                .map_err(|erro| erro.mensagem_amigavel())
                .and_then(|cliente| {
                    let catalogo = modelos::catalogo_embutido().map_err(|erro| {
                        eprintln!("o catálogo de modelos é inválido: {erro}");
                        "Não foi possível carregar a lista de modelos.".to_owned()
                    })?;
                    Ok(ServicoDeModelos::novo(
                        cliente,
                        Arc::new(instalador::disco::EspacoReal),
                        modelos::espaco::pasta_de_modelos_do_ambiente(),
                        catalogo,
                    ))
                });
            app.manage(ModelosDoApp {
                servico,
                em_andamento: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            estado_do_banco,
            preparar_ollama,
            instalar_ollama,
            cancelar_instalacao_ollama,
            onboarding_concluido,
            concluir_onboarding,
            catalogo_de_modelos,
            avaliar_espaco_do_modelo,
            baixar_modelo,
            cancelar_download_modelo
        ])
        .build(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo Memoteca")
        .run(|app, evento| {
            // Ao fechar, encerra só o Ollama que o próprio app iniciou. Se o gerenciador
            // estiver ocupado, o Job Object do Windows encerra o processo de qualquer forma.
            if let RunEvent::Exit = evento
                && let Some(ollama) = app.try_state::<OllamaDoApp>()
                && let Ok(gerenciador) = ollama.0.as_ref()
                && let Ok(mut gerenciador) = gerenciador.try_lock()
            {
                gerenciador.encerrar_se_iniciado_pelo_app();
            }
        });
}
