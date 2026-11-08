pub mod banco;
pub mod ollama;

use std::sync::Mutex;

use banco::Banco;
use ollama::ciclo::{EstadoParaInterface, GerenciadorOllama};
use tauri::{Manager, RunEvent};

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![estado_do_banco, preparar_ollama])
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
