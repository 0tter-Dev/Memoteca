pub mod banco;
pub mod catalogo;
pub mod conversas;
pub mod instalador;
pub mod intercambio;
pub mod log;
pub mod modelos;
pub mod ollama;

use std::sync::{Arc, Mutex};

use banco::intercambio::{Backup, ResumoDaImportacao};
use banco::{
    Banco, Conversa, DadosConversa, DadosProjeto, ErroBanco, EscopoMemoria, Memoria, Mensagem,
    Projeto, ResumoExclusaoProjeto,
};
use conversas::{
    AvisoDeContexto, ControleDeGeracao, DesfechoDaGeracao, DesfechoDaSugestao, ModoDeGeracao,
    SugestaoDeMemoria, gerar, registrar_mensagem_do_usuario, sugerir,
};
use instalador::{InstaladorDoOllama, ProgressoInstalacao, ResultadoInstalacao};
use intercambio::ResumoDaExportacao;
use modelos::{
    AvaliacaoParaInterface, Catalogo, ProgressoDoModelo, ResultadoDoModelo, ServicoDeModelos,
};
use ollama::ciclo::{EstadoParaInterface, GerenciadorOllama};
use ollama::instalacao::DetectorReal;
use ollama::{ClienteOllama, PedacoChat};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, RunEvent};
use tauri_plugin_dialog::DialogExt;
use tokio_util::sync::CancellationToken;

/// O banco de dados do app. Em caso de falha ao abrir guarda a mensagem amigável, que a
/// interface mostra ao usuário. Como o que está aqui pode ser trocado (`tentar_abrir_banco`),
/// o usuário se recupera de um banco que não abriu **sem precisar reiniciar o app**.
struct EstadoBanco {
    atual: Mutex<Result<Arc<Mutex<Banco>>, String>>,
}

impl EstadoBanco {
    fn novo(resultado: Result<Mutex<Banco>, String>) -> Self {
        Self {
            atual: Mutex::new(resultado.map(Arc::new)),
        }
    }

    /// O banco aberto, ou a mensagem amigável de por que não abriu.
    fn obter(&self) -> Result<Arc<Mutex<Banco>>, String> {
        self.atual
            .lock()
            .map_err(|_| ErroBanco::Interno("estado do banco".into()).mensagem_amigavel())?
            .clone()
    }

    /// Tenta abrir o banco de novo se ele não está aberto. Se já está, não chama `abrir`.
    fn tentar_reabrir(
        &self,
        abrir: impl FnOnce() -> Result<Mutex<Banco>, String>,
    ) -> Result<(), String> {
        let mut atual = self
            .atual
            .lock()
            .map_err(|_| ErroBanco::Interno("estado do banco".into()).mensagem_amigavel())?;
        if atual.is_ok() {
            return Ok(());
        }
        let novo = abrir().map(Arc::new);
        let resultado = novo.as_ref().map(|_| ()).map_err(Clone::clone);
        *atual = novo;
        resultado
    }
}

/// A interface chama este comando ao abrir. `Ok` significa que os dados locais estão
/// prontos, e `Err` traz a mensagem amigável em pt-BR.
#[tauri::command]
fn estado_do_banco(estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    estado.obter().map(|_| ())
}

/// "Tentar de novo" depois de o banco não abrir: tenta abrir outra vez, sem reiniciar o app.
/// Se já está aberto, não faz nada.
#[tauri::command]
fn tentar_abrir_banco(
    app: tauri::AppHandle,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<(), String> {
    estado.tentar_reabrir(|| abrir_banco(&app))
}

/// Abre a pasta dos dados (o banco, as cópias de segurança e o log) no Explorador de
/// Arquivos, para o usuário achar o que precisa quando o banco não abre.
#[tauri::command]
fn abrir_pasta_de_dados(app: tauri::AppHandle) -> Result<(), String> {
    let mensagem = "Não foi possível abrir a pasta dos dados. Abra o Explorador de Arquivos e procure a pasta do Memoteca dentro de AppData.";
    let pasta = app.path().app_data_dir().map_err(|erro| {
        crate::registrar!("não foi possível localizar a pasta de dados: {erro}");
        mensagem.to_owned()
    })?;
    std::fs::create_dir_all(&pasta).map_err(|erro| {
        crate::registrar!("não foi possível criar a pasta de dados: {erro}");
        mensagem.to_owned()
    })?;
    abrir_no_explorador(&pasta).map_err(|erro| {
        crate::registrar!("não foi possível abrir a pasta de dados: {erro}");
        mensagem.to_owned()
    })
}

#[cfg(windows)]
fn abrir_no_explorador(pasta: &std::path::Path) -> std::io::Result<()> {
    let mut filho = std::process::Command::new("explorer").arg(pasta).spawn()?;
    // O Explorador devolve um código de saída qualquer. Só recolhe o processo.
    std::thread::spawn(move || {
        let _ = filho.wait();
    });
    Ok(())
}

#[cfg(not(windows))]
fn abrir_no_explorador(_pasta: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other("só há suporte no Windows"))
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
            return Err(
                "Já há uma instalação do Ollama em andamento. Espere ela terminar.".to_owned(),
            );
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
            crate::registrar!("a instalação do Ollama falhou: {falha}");
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

/// Roda uma operação no banco, devolvendo só a mensagem amigável se falhar. O texto técnico
/// vai para o log de erro padrão.
fn com_banco<T>(
    estado: &EstadoBanco,
    operacao: impl FnOnce(&Banco) -> Result<T, ErroBanco>,
) -> Result<T, String> {
    let banco = estado.obter()?;
    let banco = banco
        .lock()
        .map_err(|_| ErroBanco::Interno("banco em uso".into()).mensagem_amigavel())?;
    operacao(&banco).map_err(|erro| {
        crate::registrar!("operação no banco falhou: {erro}");
        erro.mensagem_amigavel()
    })
}

/// Projetos, em ordem alfabética.
#[tauri::command]
fn listar_projetos(estado: tauri::State<'_, EstadoBanco>) -> Result<Vec<Projeto>, String> {
    com_banco(&estado, Banco::listar_projetos)
}

/// Cria um projeto. O nome é obrigatório.
#[tauri::command]
fn criar_projeto(
    dados: DadosProjeto,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Projeto, String> {
    com_banco(&estado, |banco| banco.criar_projeto(&dados))
}

/// Edita nome, instruções e modelo padrão de um projeto.
#[tauri::command]
fn atualizar_projeto(
    id: String,
    dados: DadosProjeto,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Projeto, String> {
    com_banco(&estado, |banco| banco.atualizar_projeto(&id, &dados))
}

/// O que será apagado junto com o projeto, para a tela de confirmação.
#[tauri::command]
fn resumo_exclusao_projeto(
    id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<ResumoExclusaoProjeto, String> {
    com_banco(&estado, |banco| banco.resumo_exclusao_projeto(&id))
}

/// Exclui o projeto e, em cascata, as conversas, mensagens e memórias dele. A tela só chama
/// isto depois de o usuário confirmar.
#[tauri::command]
fn excluir_projeto(id: String, estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    com_banco(&estado, |banco| banco.excluir_projeto(&id))
}

// ----- Conversas e chat -----

/// Controle de "uma geração por vez", compartilhado pelos comandos de conversa.
struct ConversasDoApp {
    controle: ControleDeGeracao,
    /// Cliente do Ollama local. Em caso de falha ao criar guarda a mensagem amigável.
    cliente: Result<ClienteOllama, String>,
}

/// Um pedaço da resposta, enviado à tela na ordem em que chega do modelo.
#[derive(Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
enum EventoDeGeracao {
    Raciocinio { texto: String },
    Conteudo { texto: String },
}

/// Como uma geração terminou, para a tela.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultadoDaGeracaoParaInterface {
    /// `concluida` ou `cancelada`.
    estado: &'static str,
    /// A resposta gravada (inteira ou interrompida), se houver.
    mensagem: Option<Mensagem>,
    /// O modelo parou por limite de tamanho.
    cortada: bool,
    /// O contexto enviado estava perto do `num_ctx` (ou além dele).
    contexto: Option<AvisoDeContexto>,
}

/// Falha da geração. A mensagem do usuário continua gravada.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FalhaDaGeracaoParaInterface {
    mensagem: String,
    /// A resposta que chegou pela metade e foi guardada como interrompida, se houver.
    mensagem_parcial: Option<Box<Mensagem>>,
}

impl FalhaDaGeracaoParaInterface {
    fn simples(mensagem: impl Into<String>) -> Self {
        Self {
            mensagem: mensagem.into(),
            mensagem_parcial: None,
        }
    }
}

/// Grava a mensagem do usuário (se houver) e gera a resposta, enviando os pedaços pelo canal.
/// A mensagem do usuário é gravada **antes** de qualquer chamada ao Ollama.
async fn executar_geracao(
    banco: &EstadoBanco,
    conversas: &ConversasDoApp,
    ollama_do_app: &OllamaDoApp,
    conversa_id: &str,
    texto: Option<&str>,
    modo: ModoDeGeracao,
    canal: &Channel<EventoDeGeracao>,
) -> Result<ResultadoDaGeracaoParaInterface, FalhaDaGeracaoParaInterface> {
    let banco = banco
        .obter()
        .map_err(FalhaDaGeracaoParaInterface::simples)?;
    let vaga = conversas
        .controle
        .iniciar()
        .map_err(|erro| FalhaDaGeracaoParaInterface::simples(erro.mensagem_amigavel()))?;

    if let Some(texto) = texto {
        registrar_mensagem_do_usuario(&banco, conversa_id, texto).map_err(|erro| {
            crate::registrar!("não foi possível gravar a mensagem: {erro}");
            FalhaDaGeracaoParaInterface::simples(erro.mensagem_amigavel())
        })?;
    }

    // Garante que o Ollama está no ar. Se ele caiu, é aqui que volta.
    let gerenciador = ollama_do_app
        .0
        .as_ref()
        .map_err(|mensagem| FalhaDaGeracaoParaInterface::simples(mensagem.clone()))?;
    let situacao = gerenciador.lock().await.preparar().await;
    if !matches!(situacao, ollama::ciclo::EstadoOllama::EmExecucao(_)) {
        let mensagem = situacao
            .para_interface()
            .mensagem
            .unwrap_or_else(|| "O Ollama não está disponível. Tente de novo.".to_owned());
        return Err(FalhaDaGeracaoParaInterface::simples(mensagem));
    }

    let cliente = conversas
        .cliente
        .as_ref()
        .map_err(|mensagem| FalhaDaGeracaoParaInterface::simples(mensagem.clone()))?;
    let resultado = gerar(
        &banco,
        cliente,
        conversa_id,
        modo,
        vaga.cancelamento(),
        &mut |pedaco| {
            let evento = match pedaco {
                PedacoChat::Raciocinio(texto) => EventoDeGeracao::Raciocinio { texto },
                PedacoChat::Conteudo(texto) => EventoDeGeracao::Conteudo { texto },
            };
            let _ = canal.send(evento);
        },
    )
    .await;

    match resultado {
        Ok(DesfechoDaGeracao::Concluida {
            mensagem,
            cortada,
            contexto,
        }) => Ok(ResultadoDaGeracaoParaInterface {
            estado: "concluida",
            mensagem: Some(mensagem),
            cortada,
            contexto,
        }),
        Ok(DesfechoDaGeracao::Cancelada { mensagem }) => Ok(ResultadoDaGeracaoParaInterface {
            estado: "cancelada",
            mensagem,
            cortada: false,
            contexto: None,
        }),
        Err(falha) => {
            crate::registrar!("a geração da resposta falhou: {}", falha.erro);
            Err(FalhaDaGeracaoParaInterface {
                mensagem: falha.erro.mensagem_amigavel(),
                mensagem_parcial: falha.mensagem_parcial,
            })
        }
    }
}

/// Conversas de um projeto, da mais recente para a mais antiga.
#[tauri::command]
fn listar_conversas(
    projeto_id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Vec<Conversa>, String> {
    com_banco(&estado, |banco| {
        banco.listar_conversas_do_projeto(&projeto_id)
    })
}

/// Cria uma conversa nova dentro do projeto, com o modelo herdado e os valores padrão.
#[tauri::command]
fn criar_conversa(
    projeto_id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Conversa, String> {
    com_banco(&estado, |banco| banco.criar_conversa_nova(&projeto_id))
}

/// Mensagens da conversa, na ordem em que foram trocadas.
#[tauri::command]
fn listar_mensagens(
    conversa_id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Vec<Mensagem>, String> {
    com_banco(&estado, |banco| banco.listar_mensagens(&conversa_id))
}

/// Edita título, modelo e parâmetros de uma conversa.
#[tauri::command]
fn atualizar_conversa(
    id: String,
    dados: DadosConversa,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Conversa, String> {
    com_banco(&estado, |banco| banco.atualizar_conversa(&id, &dados))
}

/// Quantas mensagens serão apagadas junto com a conversa.
#[tauri::command]
fn resumo_exclusao_conversa(
    id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<u64, String> {
    com_banco(&estado, |banco| banco.contar_mensagens(&id))
}

/// Exclui a conversa e as mensagens dela. A tela só chama isto depois de confirmar.
#[tauri::command]
fn excluir_conversa(id: String, estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    com_banco(&estado, |banco| banco.excluir_conversa(&id))
}

/// Grava a mensagem do usuário e gera a resposta, com streaming pelo `canal`.
#[tauri::command]
async fn enviar_mensagem(
    conversa_id: String,
    texto: String,
    canal: Channel<EventoDeGeracao>,
    banco: tauri::State<'_, EstadoBanco>,
    conversas: tauri::State<'_, ConversasDoApp>,
    ollama_do_app: tauri::State<'_, OllamaDoApp>,
) -> Result<ResultadoDaGeracaoParaInterface, FalhaDaGeracaoParaInterface> {
    executar_geracao(
        &banco,
        &conversas,
        &ollama_do_app,
        &conversa_id,
        Some(&texto),
        ModoDeGeracao::ResponderUltimaMensagem,
        &canal,
    )
    .await
}

/// "Tentar de novo": responde à última mensagem do usuário, que ficou sem resposta.
#[tauri::command]
async fn gerar_resposta_pendente(
    conversa_id: String,
    canal: Channel<EventoDeGeracao>,
    banco: tauri::State<'_, EstadoBanco>,
    conversas: tauri::State<'_, ConversasDoApp>,
    ollama_do_app: tauri::State<'_, OllamaDoApp>,
) -> Result<ResultadoDaGeracaoParaInterface, FalhaDaGeracaoParaInterface> {
    executar_geracao(
        &banco,
        &conversas,
        &ollama_do_app,
        &conversa_id,
        None,
        ModoDeGeracao::ResponderUltimaMensagem,
        &canal,
    )
    .await
}

/// Gera de novo a última resposta, que é substituída pela nova.
#[tauri::command]
async fn regenerar_resposta(
    conversa_id: String,
    canal: Channel<EventoDeGeracao>,
    banco: tauri::State<'_, EstadoBanco>,
    conversas: tauri::State<'_, ConversasDoApp>,
    ollama_do_app: tauri::State<'_, OllamaDoApp>,
) -> Result<ResultadoDaGeracaoParaInterface, FalhaDaGeracaoParaInterface> {
    executar_geracao(
        &banco,
        &conversas,
        &ollama_do_app,
        &conversa_id,
        None,
        ModoDeGeracao::RegenerarUltimaResposta,
        &canal,
    )
    .await
}

/// Cancela a geração em andamento. Não faz nada se não houver nenhuma.
#[tauri::command]
fn cancelar_geracao(conversas: tauri::State<'_, ConversasDoApp>) {
    conversas.controle.cancelar();
}

/// Janela de contexto e suporte a raciocínio de um modelo, para a tela de parâmetros.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DetalhesDoModeloParaInterface {
    janela_de_contexto: Option<u64>,
    suporta_raciocinio: bool,
}

#[tauri::command]
async fn detalhes_do_modelo(
    nome: String,
    modelos: tauri::State<'_, ModelosDoApp>,
) -> Result<DetalhesDoModeloParaInterface, String> {
    let servico = modelos.servico.as_ref().map_err(Clone::clone)?;
    let detalhes = servico
        .detalhes(&nome)
        .await
        .map_err(|erro| erro.mensagem_amigavel())?;
    Ok(DetalhesDoModeloParaInterface {
        janela_de_contexto: detalhes.janela_de_contexto,
        suporta_raciocinio: detalhes.suporta_raciocinio(),
    })
}

// ----- Memórias -----

/// Memórias globais, que valem para qualquer projeto, da mais antiga para a mais nova.
#[tauri::command]
fn listar_memorias_globais(estado: tauri::State<'_, EstadoBanco>) -> Result<Vec<Memoria>, String> {
    com_banco(&estado, Banco::listar_memorias_globais)
}

/// Memórias de um projeto, da mais antiga para a mais nova.
#[tauri::command]
fn listar_memorias_do_projeto(
    projeto_id: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Vec<Memoria>, String> {
    com_banco(&estado, |banco| {
        banco.listar_memorias_do_projeto(&projeto_id)
    })
}

/// Cria uma memória global ou de um projeto. Entra no contexto das próximas mensagens enviadas.
#[tauri::command]
fn criar_memoria(
    escopo: EscopoMemoria,
    projeto_id: Option<String>,
    conteudo: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Memoria, String> {
    com_banco(&estado, |banco| match (escopo, projeto_id) {
        (EscopoMemoria::Global, _) => banco.criar_memoria_global(&conteudo),
        (EscopoMemoria::Projeto, Some(projeto_id)) => {
            banco.criar_memoria_de_projeto(&projeto_id, &conteudo)
        }
        (EscopoMemoria::Projeto, None) => Err(ErroBanco::Rejeitado(
            "Escolha o projeto desta memória.".to_owned(),
        )),
    })
}

/// Edita o texto de uma memória. Só vale para as próximas mensagens: o que já foi gerado não
/// muda.
#[tauri::command]
fn atualizar_memoria(
    id: String,
    conteudo: String,
    estado: tauri::State<'_, EstadoBanco>,
) -> Result<Memoria, String> {
    com_banco(&estado, |banco| banco.atualizar_memoria(&id, &conteudo))
}

/// Remove uma memória, sem aviso de "contexto mudou": as próximas mensagens simplesmente não
/// a levam mais.
#[tauri::command]
fn excluir_memoria(id: String, estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    com_banco(&estado, |banco| banco.excluir_memoria(&id))
}

/// O que a tela recebe ao pedir sugestões de memória.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultadoDaSugestaoParaInterface {
    /// O usuário cancelou: não há sugestões e nada foi criado.
    cancelada: bool,
    sugestoes: Vec<SugestaoDeMemoria>,
}

/// Pede ao modelo da conversa sugestões de memória. **Não grava nada**: nem memória, nem
/// mensagem. Cada sugestão só vira memória quando o usuário a aprova na tela, pelo comando
/// `criar_memoria`. Usa a mesma "vaga" da geração de respostas, então não roda junto com uma.
#[tauri::command]
async fn sugerir_memorias(
    conversa_id: String,
    banco: tauri::State<'_, EstadoBanco>,
    conversas: tauri::State<'_, ConversasDoApp>,
    ollama_do_app: tauri::State<'_, OllamaDoApp>,
) -> Result<ResultadoDaSugestaoParaInterface, String> {
    let banco = banco.obter()?;
    let vaga = conversas
        .controle
        .iniciar()
        .map_err(|erro| erro.mensagem_amigavel())?;

    // Garante que o Ollama está no ar, como no chat.
    let gerenciador = ollama_do_app.0.as_ref().map_err(Clone::clone)?;
    let situacao = gerenciador.lock().await.preparar().await;
    if !matches!(situacao, ollama::ciclo::EstadoOllama::EmExecucao(_)) {
        return Err(situacao
            .para_interface()
            .mensagem
            .unwrap_or_else(|| "O Ollama não está disponível. Tente de novo.".to_owned()));
    }

    let cliente = conversas.cliente.as_ref().map_err(Clone::clone)?;
    match sugerir(&banco, cliente, &conversa_id, vaga.cancelamento()).await {
        Ok(DesfechoDaSugestao::Sugestoes(sugestoes)) => Ok(ResultadoDaSugestaoParaInterface {
            cancelada: false,
            sugestoes,
        }),
        Ok(DesfechoDaSugestao::Cancelada) => Ok(ResultadoDaSugestaoParaInterface {
            cancelada: true,
            sugestoes: Vec::new(),
        }),
        Err(erro) => {
            crate::registrar!("a sugestão de memórias falhou: {erro}");
            Err(erro.mensagem_amigavel())
        }
    }
}

// ----- Exportar e importar (backup manual) -----

/// O backup que o usuário escolheu e que espera a confirmação para ser importado. Fica no
/// backend, então o que é confirmado é exatamente o que foi analisado.
struct ImportacaoPendente(Mutex<Option<Backup>>);

/// Abre a janela do Windows para escolher onde salvar (`Some(nome)`) ou qual arquivo abrir
/// (`None`). `None` no resultado significa que o usuário fechou a janela sem escolher.
async fn escolher_arquivo(
    app: &tauri::AppHandle,
    janela: &tauri::WebviewWindow,
    salvar_como: Option<String>,
) -> Option<std::path::PathBuf> {
    let (envio, recebimento) = tokio::sync::oneshot::channel();
    let dialogo = app
        .dialog()
        .file()
        .set_parent(janela)
        .add_filter("Backup do Memoteca", &["json"]);
    match salvar_como {
        Some(nome) => dialogo
            .set_title("Exportar backup")
            .set_file_name(nome)
            .save_file(move |caminho| {
                let _ = envio.send(caminho);
            }),
        None => dialogo
            .set_title("Importar backup")
            .pick_file(move |caminho| {
                let _ = envio.send(caminho);
            }),
    }
    recebimento
        .await
        .ok()
        .flatten()
        .and_then(|caminho| caminho.into_path().ok())
}

/// O que a tela recebe depois de exportar.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultadoDaExportacaoParaInterface {
    /// O usuário fechou a janela sem escolher onde salvar. Nada foi gravado.
    cancelado: bool,
    caminho: Option<String>,
    resumo: Option<ResumoDaExportacao>,
}

/// Roda uma operação no banco com o bloqueio, devolvendo só a mensagem amigável se falhar.
fn com_banco_exclusivo<T>(
    banco: &Mutex<Banco>,
    operacao: impl FnOnce(&Banco) -> Result<T, ErroBanco>,
) -> Result<T, String> {
    let banco = banco
        .lock()
        .map_err(|_| ErroBanco::Interno("banco em uso".into()).mensagem_amigavel())?;
    operacao(&banco).map_err(|erro| {
        crate::registrar!("operação no banco falhou: {erro}");
        erro.mensagem_amigavel()
    })
}

/// Exporta todos os dados para um arquivo JSON. O usuário escolhe o destino.
#[tauri::command]
async fn exportar_backup(
    app: tauri::AppHandle,
    janela: tauri::WebviewWindow,
    banco: tauri::State<'_, EstadoBanco>,
) -> Result<ResultadoDaExportacaoParaInterface, String> {
    let banco = banco.obter()?;
    let data = com_banco_exclusivo(&banco, Banco::data_local_de_hoje)?;
    let Some(destino) =
        escolher_arquivo(&app, &janela, Some(format!("memoteca-backup-{data}.json"))).await
    else {
        return Ok(ResultadoDaExportacaoParaInterface {
            cancelado: true,
            caminho: None,
            resumo: None,
        });
    };

    // A leitura do banco é rápida e o bloqueio é solto antes de gravar o arquivo.
    let backup = com_banco_exclusivo(&banco, Banco::montar_backup)?;
    let resumo = intercambio::gravar_backup(&backup, &destino).map_err(|erro| {
        crate::registrar!("a exportação falhou: {erro}");
        erro.mensagem_amigavel()
    })?;
    Ok(ResultadoDaExportacaoParaInterface {
        cancelado: false,
        caminho: Some(destino.display().to_string()),
        resumo: Some(resumo),
    })
}

/// O que a tela mostra para o usuário confirmar a importação.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnaliseDoBackupParaInterface {
    caminho: String,
    /// Quando o backup foi feito.
    exportado_em: String,
    /// O que a importação faria, tipo por tipo. Nada foi gravado ainda.
    resumo: ResumoDaImportacao,
}

/// Deixa o usuário escolher um arquivo de backup, **verifica** o arquivo e diz o que a
/// importação faria. Não grava nada no banco. `None` se o usuário fechou a janela sem escolher.
#[tauri::command]
async fn analisar_backup(
    app: tauri::AppHandle,
    janela: tauri::WebviewWindow,
    banco: tauri::State<'_, EstadoBanco>,
    pendente: tauri::State<'_, ImportacaoPendente>,
) -> Result<Option<AnaliseDoBackupParaInterface>, String> {
    let banco = banco.obter()?;
    let Some(caminho) = escolher_arquivo(&app, &janela, None).await else {
        return Ok(None);
    };

    // O arquivo é lido e verificado fora do bloqueio do banco.
    let lido = {
        let caminho = caminho.clone();
        tokio::task::spawn_blocking(move || intercambio::ler_backup(&caminho))
            .await
            .map_err(|_| "Não foi possível ler o arquivo. Tente de novo.".to_owned())?
    };
    let backup = lido.map_err(|erro| {
        crate::registrar!("o backup escolhido foi recusado: {erro}");
        erro.mensagem_amigavel()
    })?;

    let resumo = {
        let banco = banco
            .lock()
            .map_err(|_| ErroBanco::Interno("banco em uso".into()).mensagem_amigavel())?;
        banco.analisar_backup(&backup).map_err(|erro| {
            crate::registrar!("a análise do backup falhou: {erro}");
            erro.mensagem_amigavel()
        })?
    };

    let exportado_em = backup.exportado_em.clone();
    *pendente.0.lock().map_err(|_| "Tente de novo.".to_owned())? = Some(backup);
    Ok(Some(AnaliseDoBackupParaInterface {
        caminho: caminho.display().to_string(),
        exportado_em,
        resumo,
    }))
}

/// Importa o backup que foi analisado e confirmado. A importação é atômica: se falhar, nada
/// fica importado pela metade.
#[tauri::command]
fn importar_backup_analisado(
    banco: tauri::State<'_, EstadoBanco>,
    pendente: tauri::State<'_, ImportacaoPendente>,
) -> Result<ResumoDaImportacao, String> {
    let banco = banco.obter()?;
    let backup = pendente
        .0
        .lock()
        .map_err(|_| "Tente de novo.".to_owned())?
        .take()
        .ok_or_else(|| "Escolha um arquivo de backup antes de importar.".to_owned())?;
    let banco = banco
        .lock()
        .map_err(|_| ErroBanco::Interno("banco em uso".into()).mensagem_amigavel())?;
    banco.importar_backup(&backup).map_err(|erro| {
        crate::registrar!("a importação falhou: {erro}");
        erro.mensagem_amigavel()
    })
}

/// O usuário desistiu da importação depois de ver o resumo.
#[tauri::command]
fn descartar_backup_analisado(pendente: tauri::State<'_, ImportacaoPendente>) {
    if let Ok(mut pendente) = pendente.0.lock() {
        *pendente = None;
    }
}

/// Chave de configuração que marca o onboarding como concluído.
const CONFIG_ONBOARDING_CONCLUIDO: &str = "onboarding_concluido";

/// O usuário já concluiu o onboarding?
#[tauri::command]
fn onboarding_concluido(estado: tauri::State<'_, EstadoBanco>) -> Result<bool, String> {
    let banco = estado.obter()?;
    let banco = banco.lock().map_err(|_| "Tente de novo.".to_owned())?;
    banco
        .obter_configuracao(CONFIG_ONBOARDING_CONCLUIDO)
        .map(|valor| valor.as_deref() == Some("sim"))
        .map_err(|erro| erro.mensagem_amigavel())
}

/// Marca o onboarding como concluído.
#[tauri::command]
fn concluir_onboarding(estado: tauri::State<'_, EstadoBanco>) -> Result<(), String> {
    let banco = estado.obter()?;
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
                "Já há um download de modelo em andamento. Espere ele terminar ou cancele.",
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
            crate::registrar!("o download do modelo falhou: {erro}");
            Err(FalhaDoDownloadParaInterface {
                mensagem: erro.mensagem_amigavel(),
                pode_continuar: erro.pode_continuar(),
            })
        }
    }
}

/// Nomes dos modelos instalados, para escolher o modelo padrão de um projeto.
#[tauri::command]
async fn listar_modelos_instalados(
    estado: tauri::State<'_, ModelosDoApp>,
) -> Result<Vec<String>, String> {
    let servico = estado.servico.as_ref().map_err(Clone::clone)?;
    servico.listar_instalados().await.map_err(|erro| {
        crate::registrar!("não foi possível listar os modelos instalados: {erro}");
        erro.mensagem_amigavel()
    })
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
        crate::registrar!("não foi possível localizar a pasta de dados: {erro}");
        banco::ErroBanco::Indisponivel(erro.to_string()).mensagem_amigavel()
    })?;

    Banco::abrir(&pasta.join("memoteca.db"), &pasta.join("backups"))
        .map(Mutex::new)
        .map_err(|erro| {
            crate::registrar!("não foi possível abrir o banco de dados: {erro}");
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Ok(pasta) = app.path().app_data_dir() {
                log::iniciar(&log::pasta_do_log(&pasta));
            }
            let estado = EstadoBanco::novo(abrir_banco(app.handle()));
            app.manage(estado);
            app.manage(ImportacaoPendente(Mutex::new(None)));

            let ollama = GerenciadorOllama::novo()
                .map(tokio::sync::Mutex::new)
                .map_err(|erro| {
                    crate::registrar!("não foi possível preparar o gerenciador do Ollama: {erro}");
                    erro.mensagem_amigavel()
                });
            app.manage(OllamaDoApp(ollama));

            let instalador = InstaladorDoOllama::novo(Arc::new(DetectorReal)).map_err(|erro| {
                crate::registrar!("não foi possível preparar o instalador do Ollama: {erro}");
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
                        crate::registrar!("o catálogo de modelos é inválido: {erro}");
                        "Não foi possível carregar a lista de modelos. Feche o aplicativo e abra-o de novo.".to_owned()
                    })?;
                    Ok(ServicoDeModelos::novo(
                        cliente,
                        Arc::new(instalador::disco::EspacoReal),
                        modelos::espaco::pasta_de_modelos_do_ambiente(),
                        catalogo,
                    ))
                });
            app.manage(ConversasDoApp {
                controle: ControleDeGeracao::default(),
                cliente: ClienteOllama::novo().map_err(|erro| erro.mensagem_amigavel()),
            });
            app.manage(ModelosDoApp {
                servico,
                em_andamento: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            estado_do_banco,
            tentar_abrir_banco,
            abrir_pasta_de_dados,
            preparar_ollama,
            instalar_ollama,
            cancelar_instalacao_ollama,
            onboarding_concluido,
            concluir_onboarding,
            catalogo_de_modelos,
            avaliar_espaco_do_modelo,
            baixar_modelo,
            cancelar_download_modelo,
            listar_projetos,
            criar_projeto,
            atualizar_projeto,
            resumo_exclusao_projeto,
            excluir_projeto,
            listar_modelos_instalados,
            listar_conversas,
            criar_conversa,
            listar_mensagens,
            atualizar_conversa,
            resumo_exclusao_conversa,
            excluir_conversa,
            enviar_mensagem,
            gerar_resposta_pendente,
            regenerar_resposta,
            cancelar_geracao,
            detalhes_do_modelo,
            listar_memorias_globais,
            listar_memorias_do_projeto,
            criar_memoria,
            atualizar_memoria,
            excluir_memoria,
            sugerir_memorias,
            exportar_backup,
            analisar_backup,
            importar_backup_analisado,
            descartar_backup_analisado
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

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_nova() -> std::path::PathBuf {
        let pasta =
            std::env::temp_dir().join(format!("memoteca-estado-do-banco-{}", banco::id::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    fn abrir_em(pasta: &std::path::Path) -> Result<Mutex<Banco>, String> {
        Banco::abrir(&pasta.join("memoteca.db"), &pasta.join("backups"))
            .map(Mutex::new)
            .map_err(|erro| erro.mensagem_amigavel())
    }

    /// R26: o banco não abre, o app mostra a mensagem amigável, os dados **não são apagados**
    /// e, resolvido o problema, "tentar de novo" abre o banco **sem reiniciar o app**.
    #[test]
    fn r26_banco_que_nao_abre_se_recupera_com_tentar_de_novo_sem_reiniciar() {
        let pasta = pasta_nova();
        let arquivo = pasta.join("memoteca.db");
        // Um arquivo que não é um banco de dados (ou que está danificado).
        std::fs::write(&arquivo, b"isto nao e um banco sqlite, mas e do usuario").unwrap();
        let estado = EstadoBanco::novo(abrir_em(&pasta));

        // Aberto o app: só a mensagem amigável, sem texto do SQLite.
        let erro = estado.obter().map(|_| ()).unwrap_err();
        assert!(erro.contains("Seus dados não foram apagados"), "{erro}");
        for tecnico in ["SQLITE", "sqlite", "not a database", "NotADatabase"] {
            assert!(!erro.contains(tecnico), "{erro}");
        }
        // O arquivo do usuário continua lá, intacto.
        assert_eq!(
            std::fs::read(&arquivo).unwrap(),
            b"isto nao e um banco sqlite, mas e do usuario"
        );

        // Ainda com o problema, tentar de novo falha com a mesma mensagem amigável.
        let de_novo = estado.tentar_reabrir(|| abrir_em(&pasta)).unwrap_err();
        assert!(
            de_novo.contains("Seus dados não foram apagados"),
            "{de_novo}"
        );

        // O usuário resolve (aqui, tira o arquivo do lugar) e tenta de novo.
        std::fs::rename(&arquivo, pasta.join("guardado.db")).unwrap();
        estado.tentar_reabrir(|| abrir_em(&pasta)).unwrap();

        // O app passou a funcionar na hora, sem reiniciar.
        let banco = estado.obter().unwrap();
        let banco = banco.lock().unwrap();
        banco
            .criar_projeto(&banco::DadosProjeto {
                nome: "Funciona".into(),
                system_prompt: String::new(),
                modelo_padrao: None,
            })
            .unwrap();
        assert_eq!(banco.listar_projetos().unwrap().len(), 1);
        drop(banco);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn tentar_de_novo_com_o_banco_ja_aberto_nao_abre_outro() {
        let estado = EstadoBanco::novo(Ok(Mutex::new(Banco::em_memoria().unwrap())));
        let mut chamadas = 0;

        estado
            .tentar_reabrir(|| {
                chamadas += 1;
                Err("não deveria ser chamado".to_owned())
            })
            .unwrap();

        assert_eq!(chamadas, 0);
        assert!(estado.obter().is_ok());
    }
}
