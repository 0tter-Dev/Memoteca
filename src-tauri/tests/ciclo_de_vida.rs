//! Testes de aceite da E06: ciclo de vida do Ollama.
//!
//! A maior parte usa um detector de instalação, um iniciador de processo e um servidor
//! Ollama falsos. Os testes marcados com `r04` no Windows usam **processos de verdade** para
//! provar que o Job Object encerra o Ollama junto com o app, inclusive em fechamento forçado.
//! Os testes dos cenários do plano de robustez citam o identificador `Rnn` no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use memoteca_lib::ollama::ciclo::{ConfigCiclo, EstadoOllama, GerenciadorOllama, Origem};
use memoteca_lib::ollama::instalacao::DetectorDeInstalacao;
use memoteca_lib::ollama::processo::{IniciadorDeProcesso, ProcessoFilho};
use memoteca_lib::ollama::{ClienteOllama, ErroOllama, Limites, Versao};
use ollama_falso::{OllamaFalso, Resposta, porta_livre};

// ----- Fakes -----

struct DetectorFalso(Option<PathBuf>);

impl DetectorDeInstalacao for DetectorFalso {
    fn encontrar(&self) -> Option<PathBuf> {
        self.0.clone()
    }
}

#[derive(Default)]
struct EstadoFilho {
    encerrado: AtomicBool,
    saida: Mutex<Option<i32>>,
    /// O "Ollama" que esse processo falso mantém no ar, se for o caso.
    servidor: Mutex<Option<OllamaFalso>>,
}

struct FilhoFalso(Arc<EstadoFilho>);

impl ProcessoFilho for FilhoFalso {
    fn saiu(&mut self) -> io::Result<Option<i32>> {
        Ok(*self.0.saida.lock().unwrap())
    }

    fn encerrar(&mut self) -> io::Result<()> {
        self.0.encerrado.store(true, Ordering::SeqCst);
        *self.0.saida.lock().unwrap() = Some(1);
        // Encerrar o processo derruba o servidor que ele mantinha.
        self.0.servidor.lock().unwrap().take();
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Comportamento {
    /// O processo sobe e, depois do atraso, um Ollama passa a responder na porta.
    SobeOllama { atraso: Duration },
    /// O processo sobe, mas outro programa passa a responder na porta.
    OutroProgramaNaPorta,
    /// Como o Ollama de verdade com a pasta de modelos vazia: a porta abre e responde algo que
    /// não é um Ollama por um tempo, e só depois o servidor passa a funcionar.
    PortaAbreAntesDeResponder { atraso: Duration },
    /// O processo fica vivo e nunca abre a porta.
    NuncaResponde,
    /// O processo termina na hora, com este código.
    SaiLogo(i32),
    /// Não foi possível iniciar o processo.
    NaoInicia,
}

#[derive(Default)]
struct Registro {
    iniciadas: Mutex<Vec<(PathBuf, Vec<String>)>>,
    filhos: Mutex<Vec<Arc<EstadoFilho>>>,
}

struct IniciadorFalso {
    registro: Arc<Registro>,
    comportamento: Comportamento,
    endereco: SocketAddr,
}

impl IniciadorDeProcesso for IniciadorFalso {
    fn iniciar(
        &self,
        executavel: &Path,
        argumentos: &[&str],
    ) -> io::Result<Box<dyn ProcessoFilho>> {
        self.registro.iniciadas.lock().unwrap().push((
            executavel.to_path_buf(),
            argumentos.iter().map(|a| (*a).to_owned()).collect(),
        ));

        let estado = Arc::new(EstadoFilho::default());
        match self.comportamento {
            Comportamento::NaoInicia => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "acesso negado",
                ));
            }
            Comportamento::SaiLogo(codigo) => *estado.saida.lock().unwrap() = Some(codigo),
            Comportamento::NuncaResponde => {}
            Comportamento::SobeOllama { atraso } => {
                subir_servidor(&estado, self.endereco, atraso, true);
            }
            Comportamento::OutroProgramaNaPorta => {
                subir_servidor(&estado, self.endereco, Duration::ZERO, false);
            }
            Comportamento::PortaAbreAntesDeResponder { atraso } => {
                subir_servidor_que_demora_a_responder(&estado, self.endereco, atraso);
            }
        }
        self.registro
            .filhos
            .lock()
            .unwrap()
            .push(Arc::clone(&estado));
        Ok(Box::new(FilhoFalso(estado)))
    }
}

/// Faz um servidor passar a responder em `endereco`, como se o processo tivesse subido.
fn subir_servidor(
    estado: &Arc<EstadoFilho>,
    endereco: SocketAddr,
    atraso: Duration,
    e_ollama: bool,
) {
    let estado = Arc::clone(estado);
    tokio::spawn(async move {
        tokio::time::sleep(atraso).await;
        if estado.encerrado.load(Ordering::SeqCst) {
            return;
        }
        let servidor = OllamaFalso::iniciar_em(&endereco.to_string()).await;
        if e_ollama {
            servidor.rota(
                "GET",
                "/api/version",
                Resposta::json(r#"{"version":"0.32.9"}"#),
            );
            servidor.rota(
                "GET",
                "/api/tags",
                Resposta::json(r#"{"models":[{"name":"llama3.2:latest"}]}"#),
            );
        } else {
            servidor.rota(
                "GET",
                "/api/version",
                Resposta::json("<html>outro programa</html>"),
            );
        }
        *estado.servidor.lock().unwrap() = Some(servidor);
    });
}

/// A porta abre já, respondendo algo que não é um Ollama, e só depois de `atraso` o servidor
/// passa a responder como um Ollama de verdade.
fn subir_servidor_que_demora_a_responder(
    estado: &Arc<EstadoFilho>,
    endereco: SocketAddr,
    atraso: Duration,
) {
    let estado = Arc::clone(estado);
    tokio::spawn(async move {
        let servidor = OllamaFalso::iniciar_em(&endereco.to_string()).await;
        servidor.rota(
            "GET",
            "/api/version",
            Resposta::json("<html>ainda subindo</html>"),
        );
        *estado.servidor.lock().unwrap() = Some(servidor);

        tokio::time::sleep(atraso).await;
        if let Some(servidor) = estado.servidor.lock().unwrap().as_ref() {
            servidor.rota(
                "GET",
                "/api/version",
                Resposta::json(r#"{"version":"0.32.9"}"#),
            );
            servidor.rota("GET", "/api/tags", Resposta::json(r#"{"models":[]}"#));
        }
    });
}

fn exe_falso() -> PathBuf {
    PathBuf::from(r"C:\Ollama\falso\ollama.exe")
}

/// Limites para uma porta onde ninguém ouve: no Windows a recusa leva cerca de 2 s.
fn limites() -> Limites {
    Limites {
        conexao: Duration::from_secs(4),
        consulta: Duration::from_secs(6),
        ociosidade_chat: Duration::from_secs(6),
        ociosidade_pull: Duration::from_secs(6),
    }
}

fn config(espera: Duration) -> ConfigCiclo {
    ConfigCiclo {
        espera_pela_inicializacao: espera,
        intervalo_entre_consultas: Duration::from_millis(50),
    }
}

fn montar(
    endereco: SocketAddr,
    instalado: bool,
    comportamento: Comportamento,
    espera: Duration,
) -> (GerenciadorOllama, Arc<Registro>) {
    let registro = Arc::new(Registro::default());
    let gerenciador = GerenciadorOllama::com_componentes(
        ClienteOllama::com_endereco(&format!("http://{endereco}"), limites()).unwrap(),
        Box::new(DetectorFalso(instalado.then(exe_falso))),
        Box::new(IniciadorFalso {
            registro: Arc::clone(&registro),
            comportamento,
            endereco,
        }),
        config(espera),
    );
    (gerenciador, registro)
}

fn servidor_ollama(versao: &str, modelos: &str) -> impl std::future::Future<Output = OllamaFalso> {
    let versao = versao.to_owned();
    let modelos = modelos.to_owned();
    async move {
        let servidor = OllamaFalso::iniciar().await;
        servidor.rota(
            "GET",
            "/api/version",
            Resposta::json(&format!(r#"{{"version":"{versao}"}}"#)),
        );
        servidor.rota("GET", "/api/tags", Resposta::json(&modelos));
        servidor
    }
}

fn endereco_de(servidor: &OllamaFalso) -> SocketAddr {
    servidor
        .url()
        .trim_start_matches("http://")
        .parse()
        .expect("endereço do servidor falso")
}

const COM_UM_MODELO: &str = r#"{"models":[{"name":"llama3.2:latest"}]}"#;
const SEM_MODELOS: &str = r#"{"models":[]}"#;

// ----- Reutilizar um Ollama que já está rodando (R03) -----

#[tokio::test]
async fn r03_com_um_ollama_externo_o_app_o_usa_e_nunca_o_encerra() {
    let servidor = servidor_ollama("0.32.9", COM_UM_MODELO).await;
    let (mut gerenciador, registro) = montar(
        endereco_de(&servidor),
        true,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = estado else {
        panic!("deveria estar em execução: {estado:?}");
    };
    assert_eq!(situacao.origem, Origem::Externo);
    assert_eq!(situacao.versao.versao, Versao::nova(0, 32, 9));
    assert_eq!(situacao.modelos_instalados, 1);
    assert!(
        registro.iniciadas.lock().unwrap().is_empty(),
        "nenhum processo deve ser iniciado"
    );
    assert!(!gerenciador.iniciado_pelo_app());

    // Fechar o app não mexe no Ollama externo: ele continua respondendo.
    gerenciador.encerrar_se_iniciado_pelo_app();
    drop(gerenciador);
    let cliente = ClienteOllama::com_endereco(&servidor.url(), limites()).unwrap();
    assert!(
        cliente.verificar().await.is_ok(),
        "o Ollama externo segue rodando"
    );
}

// ----- Porta ocupada por outro programa (R02) -----

#[tokio::test]
async fn r02_com_outro_programa_na_porta_nada_e_iniciado_e_a_mensagem_e_amigavel() {
    let outro = OllamaFalso::iniciar().await;
    outro.rota(
        "GET",
        "/api/version",
        Resposta::json("<html>servidor web qualquer</html>"),
    );
    let (mut gerenciador, registro) = montar(
        endereco_de(&outro),
        true,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    assert_eq!(estado, EstadoOllama::PortaOcupada);
    assert!(registro.iniciadas.lock().unwrap().is_empty());
    assert!(!gerenciador.iniciado_pelo_app());
    let dto = estado.para_interface();
    assert_eq!(dto.estado, "porta_ocupada");
    assert!(dto.mensagem.unwrap().contains("Outro programa"));
}

// ----- Sem Ollama instalado (R01) -----

#[tokio::test]
async fn r01_sem_ollama_instalado_o_estado_e_nao_instalado_e_nada_e_iniciado() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        false,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    assert_eq!(estado, EstadoOllama::NaoInstalado);
    assert!(registro.iniciadas.lock().unwrap().is_empty());
    assert_eq!(estado.para_interface().estado, "nao_instalado");
}

// ----- Instalado e parado: o app inicia -----

#[tokio::test]
async fn instalado_e_parado_o_app_inicia_o_ollama_e_espera_ele_responder() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::SobeOllama {
            atraso: Duration::from_millis(300),
        },
        Duration::from_secs(15),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = estado else {
        panic!("deveria estar em execução: {estado:?}");
    };
    assert_eq!(situacao.origem, Origem::IniciadoPeloApp);
    assert!(gerenciador.iniciado_pelo_app());
    let iniciadas = registro.iniciadas.lock().unwrap().clone();
    assert_eq!(iniciadas.len(), 1);
    assert_eq!(iniciadas[0].0, exe_falso());
    assert_eq!(iniciadas[0].1, ["serve"]);
}

#[tokio::test]
async fn ao_fechar_o_app_encerra_o_ollama_que_ele_mesmo_iniciou() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::SobeOllama {
            atraso: Duration::ZERO,
        },
        Duration::from_secs(15),
    );
    gerenciador.preparar().await;
    let filho = Arc::clone(&registro.filhos.lock().unwrap()[0]);
    assert!(!filho.encerrado.load(Ordering::SeqCst));

    gerenciador.encerrar_se_iniciado_pelo_app();

    assert!(filho.encerrado.load(Ordering::SeqCst));
    assert!(!gerenciador.iniciado_pelo_app());
}

#[tokio::test]
async fn descartar_o_gerenciador_tambem_encerra_o_ollama_que_ele_iniciou() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::SobeOllama {
            atraso: Duration::ZERO,
        },
        Duration::from_secs(15),
    );
    gerenciador.preparar().await;
    let filho = Arc::clone(&registro.filhos.lock().unwrap()[0]);

    drop(gerenciador);

    assert!(filho.encerrado.load(Ordering::SeqCst));
}

#[tokio::test]
async fn preparar_de_novo_nao_inicia_um_segundo_processo() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::SobeOllama {
            atraso: Duration::ZERO,
        },
        Duration::from_secs(15),
    );

    gerenciador.preparar().await;
    let segundo = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = segundo else {
        panic!("deveria continuar em execução: {segundo:?}");
    };
    assert_eq!(situacao.origem, Origem::IniciadoPeloApp);
    assert_eq!(registro.iniciadas.lock().unwrap().len(), 1);
}

// ----- Apoio dos testes -----

/// Os testes rodam em paralelo e cada um pede a sua porta. Duas portas iguais faziam um teste
/// enxergar o servidor do outro (falha intermitente no CI), então a mesma porta nunca pode ser
/// entregue duas vezes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn portas_livres_pedidas_ao_mesmo_tempo_nunca_se_repetem() {
    let pedidos: Vec<_> = (0..300)
        .map(|_| tokio::spawn(async { porta_livre().await }))
        .collect();

    let mut portas = std::collections::HashSet::new();
    for pedido in pedidos {
        let endereco = pedido.await.unwrap();
        assert!(
            portas.insert(endereco.port()),
            "a porta {} foi entregue duas vezes",
            endereco.port()
        );
    }
    assert_eq!(portas.len(), 300);
}

// ----- Ollama que demora a responder depois de abrir a porta -----

#[tokio::test]
async fn a_porta_aberta_antes_de_o_ollama_responder_nao_e_confundida_com_outro_programa() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::PortaAbreAntesDeResponder {
            atraso: Duration::from_millis(1500),
        },
        Duration::from_secs(20),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = estado else {
        panic!("deveria esperar o Ollama responder: {estado:?}");
    };
    assert_eq!(situacao.origem, Origem::IniciadoPeloApp);
    assert_eq!(registro.iniciadas.lock().unwrap().len(), 1);
    let filho = Arc::clone(&registro.filhos.lock().unwrap()[0]);
    assert!(
        !filho.encerrado.load(Ordering::SeqCst),
        "o processo que ainda estava subindo não pode ser encerrado"
    );
}

// ----- Falhas ao iniciar -----

#[tokio::test]
async fn nao_conseguir_iniciar_o_processo_da_mensagem_amigavel_sem_texto_tecnico() {
    let (mut gerenciador, _) = montar(
        porta_livre().await,
        true,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::Indisponivel(erro) = &estado else {
        panic!("deveria estar indisponível: {estado:?}");
    };
    assert!(matches!(erro, ErroOllama::FalhaAoIniciar(_)));
    let mensagem = estado.para_interface().mensagem.unwrap();
    assert!(mensagem.contains("iniciar o Ollama"));
    assert!(!mensagem.contains("acesso negado"));
    assert!(!gerenciador.iniciado_pelo_app());
}

#[tokio::test]
async fn processo_que_termina_logo_depois_de_iniciar_vira_falha_ao_iniciar() {
    let (mut gerenciador, _) = montar(
        porta_livre().await,
        true,
        Comportamento::SaiLogo(1),
        Duration::from_secs(15),
    );

    let estado = gerenciador.preparar().await;

    assert!(matches!(
        estado,
        EstadoOllama::Indisponivel(ErroOllama::FalhaAoIniciar(_))
    ));
    assert!(!gerenciador.iniciado_pelo_app());
}

#[tokio::test]
async fn processo_que_nunca_abre_a_porta_estoura_o_tempo_e_e_encerrado() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::NuncaResponde,
        Duration::from_secs(1),
    );

    let estado = gerenciador.preparar().await;

    assert_eq!(
        estado,
        EstadoOllama::Indisponivel(ErroOllama::TempoEsgotado)
    );
    let filho = Arc::clone(&registro.filhos.lock().unwrap()[0]);
    assert!(
        filho.encerrado.load(Ordering::SeqCst),
        "o processo que não respondeu não pode ficar rodando"
    );
}

#[tokio::test]
async fn r02_se_outro_programa_ocupa_a_porta_depois_de_iniciar_o_app_desfaz_o_que_iniciou() {
    let (mut gerenciador, registro) = montar(
        porta_livre().await,
        true,
        Comportamento::OutroProgramaNaPorta,
        Duration::from_secs(1),
    );

    let estado = gerenciador.preparar().await;

    assert_eq!(estado, EstadoOllama::PortaOcupada);
    let filho = Arc::clone(&registro.filhos.lock().unwrap()[0]);
    assert!(filho.encerrado.load(Ordering::SeqCst));
    assert!(!gerenciador.iniciado_pelo_app());
}

// ----- Versão e modelos (R05) -----

#[tokio::test]
async fn r05_versao_abaixo_da_minima_mostra_aviso_amigavel_e_nao_bloqueia() {
    let servidor = servidor_ollama("0.29.9", COM_UM_MODELO).await;
    let (mut gerenciador, registro) = montar(
        endereco_de(&servidor),
        true,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = &estado else {
        panic!("a versão antiga não pode bloquear o uso: {estado:?}");
    };
    assert!(situacao.aviso_de_versao().is_some());
    let dto = estado.para_interface();
    assert_eq!(dto.estado, "em_execucao");
    let aviso = dto.aviso.expect("deve haver aviso");
    assert!(aviso.contains("0.29.9") && aviso.contains("0.30.0"));
    assert!(registro.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ollama_sem_modelos_pede_o_onboarding_de_modelo() {
    let servidor = servidor_ollama("0.32.9", SEM_MODELOS).await;
    let (mut gerenciador, _) = montar(
        endereco_de(&servidor),
        true,
        Comportamento::NaoInicia,
        Duration::from_secs(5),
    );

    let estado = gerenciador.preparar().await;

    let EstadoOllama::EmExecucao(situacao) = estado else {
        panic!("deveria estar em execução");
    };
    assert!(situacao.precisa_de_modelo());
}

// ----- Processos de verdade: o Job Object (R04) -----

#[cfg(windows)]
mod processos_reais {
    use std::fs;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use memoteca_lib::banco::id::novo_id;
    use memoteca_lib::ollama::processo::{IniciadorDeProcesso, IniciadorReal};

    /// Marca os argumentos dos processos auxiliares. O libtest trata o que não conhece como
    /// filtro de nome de teste, então o texto passa sem atrapalhar.
    const MARCA: &str = "memoteca-aux:";

    fn argumentos_auxiliares(teste: &str, arquivo_filho: &str, arquivo_neto: &str) -> Vec<String> {
        vec![
            teste.to_owned(),
            "--exact".to_owned(),
            "--nocapture".to_owned(),
            format!("{MARCA}{arquivo_filho}|{arquivo_neto}"),
        ]
    }

    fn arquivos_dos_argumentos() -> Option<(String, String)> {
        let marcado = std::env::args().find_map(|a| a.strip_prefix(MARCA).map(str::to_owned))?;
        let (filho, neto) = marcado.split_once('|')?;
        Some((filho.to_owned(), neto.to_owned()))
    }

    fn dormir_para_sempre() -> ! {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    // Papéis auxiliares. Quando o teste roda normalmente (sem a marca), não fazem nada.

    /// Faz o papel do `ollama serve`: anota o próprio PID, inicia um "executor do modelo"
    /// (neto), anota o PID dele e dorme.
    // O neto fica vivo de propósito: é o Job Object que o encerra, e é isso que se testa.
    #[allow(clippy::zombie_processes)]
    #[test]
    fn auxiliar_filho() {
        let Some((arquivo_filho, arquivo_neto)) = arquivos_dos_argumentos() else {
            return;
        };
        let neto = Command::new(std::env::current_exe().unwrap())
            .args(["processos_reais::auxiliar_neto", "--exact", "--nocapture"])
            .arg(format!("{MARCA}{arquivo_filho}|{arquivo_neto}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        fs::write(&arquivo_neto, neto.id().to_string()).unwrap();
        fs::write(&arquivo_filho, std::process::id().to_string()).unwrap();
        dormir_para_sempre();
    }

    /// Faz o papel do executor do modelo, iniciado pelo Ollama.
    #[test]
    fn auxiliar_neto() {
        if arquivos_dos_argumentos().is_some() {
            dormir_para_sempre();
        }
    }

    /// Faz o papel do Memoteca: inicia o "Ollama" pelo `IniciadorReal` e fica rodando até
    /// ser finalizado.
    #[test]
    fn auxiliar_pai() {
        let Some((arquivo_filho, arquivo_neto)) = arquivos_dos_argumentos() else {
            return;
        };
        let argumentos = argumentos_auxiliares(
            "processos_reais::auxiliar_filho",
            &arquivo_filho,
            &arquivo_neto,
        );
        let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();
        let _filho = IniciadorReal
            .iniciar(&std::env::current_exe().unwrap(), &argumentos)
            .expect("iniciar o processo");
        dormir_para_sempre();
    }

    // Apoio dos testes

    struct Arquivos {
        pasta: std::path::PathBuf,
        filho: String,
        neto: String,
        /// PIDs a matar no fim se o teste falhar, para não deixar processos soltos.
        pids: Vec<u32>,
    }

    impl Arquivos {
        fn novos() -> Self {
            let pasta = std::env::temp_dir().join(format!("memoteca-job-{}", novo_id()));
            fs::create_dir_all(&pasta).unwrap();
            Self {
                filho: pasta.join("filho.pid").to_string_lossy().into_owned(),
                neto: pasta.join("neto.pid").to_string_lossy().into_owned(),
                pasta,
                pids: Vec::new(),
            }
        }

        /// Espera os dois arquivos de PID aparecerem e devolve `(filho, neto)`.
        fn esperar_pids(&mut self) -> (u32, u32) {
            let limite = Instant::now() + Duration::from_secs(30);
            loop {
                let filho = fs::read_to_string(&self.filho)
                    .ok()
                    .and_then(|t| t.trim().parse().ok());
                let neto = fs::read_to_string(&self.neto)
                    .ok()
                    .and_then(|t| t.trim().parse().ok());
                if let (Some(filho), Some(neto)) = (filho, neto) {
                    self.pids = vec![filho, neto];
                    return (filho, neto);
                }
                assert!(
                    Instant::now() < limite,
                    "os processos auxiliares não iniciaram"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    impl Drop for Arquivos {
        fn drop(&mut self) {
            for pid in &self.pids {
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = fs::remove_dir_all(&self.pasta);
        }
    }

    fn processo_existe(pid: u32) -> bool {
        let saida = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("rodar o tasklist");
        String::from_utf8_lossy(&saida.stdout).contains(&format!("\"{pid}\""))
    }

    fn esperar_sumir(pid: u32, limite: Duration) -> bool {
        let fim = Instant::now() + limite;
        while Instant::now() < fim {
            if !processo_existe(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        !processo_existe(pid)
    }

    #[test]
    fn r04_encerrar_normalmente_mata_o_processo_e_os_descendentes_dele() {
        let mut arquivos = Arquivos::novos();
        let argumentos = argumentos_auxiliares(
            "processos_reais::auxiliar_filho",
            &arquivos.filho,
            &arquivos.neto,
        );
        let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();

        let mut filho = IniciadorReal
            .iniciar(&std::env::current_exe().unwrap(), &argumentos)
            .expect("iniciar o processo");
        let (pid_filho, pid_neto) = arquivos.esperar_pids();
        assert!(processo_existe(pid_filho), "o processo deve estar rodando");
        assert!(
            processo_existe(pid_neto),
            "o executor do modelo deve estar rodando"
        );
        assert_eq!(filho.saiu().unwrap(), None);

        filho.encerrar().expect("encerrar");

        assert!(esperar_sumir(pid_filho, Duration::from_secs(10)));
        assert!(esperar_sumir(pid_neto, Duration::from_secs(10)));
        assert!(filho.saiu().unwrap().is_some());
        // Encerrar de novo não dá erro.
        filho.encerrar().expect("encerrar duas vezes é inofensivo");
    }

    #[test]
    fn r04_finalizar_o_app_a_forca_encerra_o_ollama_e_os_descendentes_dele() {
        let mut arquivos = Arquivos::novos();
        let argumentos = argumentos_auxiliares(
            "processos_reais::auxiliar_pai",
            &arquivos.filho,
            &arquivos.neto,
        );

        // O "app": um processo que inicia o "Ollama" e fica rodando.
        let mut app = Command::new(std::env::current_exe().unwrap())
            .args(&argumentos)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("iniciar o app falso");
        let (pid_filho, pid_neto) = arquivos.esperar_pids();
        assert!(
            processo_existe(pid_filho),
            "o Ollama falso deve estar rodando"
        );
        assert!(
            processo_existe(pid_neto),
            "o executor do modelo deve estar rodando"
        );

        // Fechamento forçado: nenhum código do app tem chance de rodar.
        app.kill().expect("finalizar o app à força");
        app.wait().unwrap();

        assert!(
            esperar_sumir(pid_filho, Duration::from_secs(15)),
            "o Ollama não pode ficar órfão depois de o app ser finalizado à força"
        );
        assert!(
            esperar_sumir(pid_neto, Duration::from_secs(15)),
            "o executor do modelo também não pode ficar órfão"
        );
    }
}
