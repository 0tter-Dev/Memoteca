//! Testes de aceite da E07: instalação silenciosa do Ollama.
//!
//! O servidor falso faz o papel do GitHub (informações da release) e do download. O instalador
//! do Ollama, a validação de assinatura, o espaço em disco e a detecção são falsos. Nenhum
//! teste instala o Ollama de verdade. Os testes dos cenários do plano de robustez citam o
//! identificador `Rnn` no nome.

#[path = "support/ollama_falso.rs"]
mod ollama_falso;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::instalador::assinatura::{FalhaDeAssinatura, ValidadorDeAssinatura};
use memoteca_lib::instalador::disco::EspacoEmDisco;
use memoteca_lib::instalador::executor::{ErroDeExecucao, ExecutorDoInstalador};
use memoteca_lib::instalador::fonte::RegrasDeOrigem;
use memoteca_lib::instalador::{
    ConfigInstalador, ESPACO_PARA_INSTALAR_EM_BYTES, ErroInstalacao, Fase, InstaladorDoOllama,
    ProgressoInstalacao, ResultadoInstalacao,
};
use memoteca_lib::ollama::instalacao::DetectorDeInstalacao;
use ollama_falso::{OllamaFalso, Passo, Resposta};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

// ----- Fakes -----

struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    fn nova() -> Self {
        let caminho = std::env::temp_dir().join(format!("memoteca-instalador-{}", novo_id()));
        fs::create_dir_all(&caminho).unwrap();
        Self(caminho)
    }

    fn download(&self) -> PathBuf {
        self.0.join("temp")
    }

    fn instalacao(&self) -> PathBuf {
        self.0.join("Programs").join("Ollama")
    }

    /// Arquivos que sobraram na pasta de download.
    fn arquivos_baixados(&self) -> Vec<String> {
        match fs::read_dir(self.download()) {
            Ok(entradas) => entradas
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Acha o `ollama.exe` na pasta de instalação, como o detector real faria.
struct DetectorNaPasta(PathBuf);

impl DetectorDeInstalacao for DetectorNaPasta {
    fn encontrar(&self) -> Option<PathBuf> {
        let executavel = self.0.join("ollama.exe");
        executavel.is_file().then_some(executavel)
    }
}

struct EspacoFalso(u64);

impl EspacoEmDisco for EspacoFalso {
    fn livre_em(&self, _pasta: &Path) -> io::Result<u64> {
        Ok(self.0)
    }
}

struct AssinaturaFalsa {
    resultado: Result<(), FalhaDeAssinatura>,
    /// Caminhos validados, e se o arquivo tinha a extensão `.exe` e existia na hora.
    chamadas: Mutex<Vec<(PathBuf, bool)>>,
}

impl ValidadorDeAssinatura for AssinaturaFalsa {
    fn validar(&self, arquivo: &Path, _publicador: &str) -> Result<(), FalhaDeAssinatura> {
        let valido = arquivo.is_file() && arquivo.extension().is_some_and(|e| e == "exe");
        self.chamadas
            .lock()
            .unwrap()
            .push((arquivo.to_path_buf(), valido));
        self.resultado.clone()
    }
}

#[derive(Clone, Copy)]
enum Comportamento {
    /// Instala: cria a pasta, o `ollama.exe` e o desinstalador.
    Sucesso,
    /// Termina bem, mas o Ollama não aparece.
    SucessoSemOllama,
    /// Deixa arquivos pela metade e sai com este código.
    FalhaNoMeio(i32),
    /// Como `FalhaNoMeio`, mas um arquivo fica travado e não dá para apagar a pasta.
    FalhaComArquivoTravado,
    /// O Windows exige elevação.
    ExigeElevacao,
    /// Fica rodando até o cancelamento, deixando arquivos pela metade.
    FicaRodando,
}

struct ExecutorFalso {
    pasta_de_instalacao: PathBuf,
    comportamento: Mutex<Comportamento>,
    /// O desinstalador remove a pasta de verdade?
    desinstalador_limpa: bool,
    eventos: Mutex<Vec<String>>,
    /// Estado do instalador na hora de rodar: existia, tamanho e SHA-256.
    arquivo_executado: Mutex<Option<(bool, String)>>,
    travado: Mutex<Option<fs::File>>,
}

impl ExecutorFalso {
    fn novo(pasta_de_instalacao: PathBuf, comportamento: Comportamento) -> Arc<Self> {
        Arc::new(Self {
            pasta_de_instalacao,
            comportamento: Mutex::new(comportamento),
            desinstalador_limpa: true,
            eventos: Mutex::new(Vec::new()),
            arquivo_executado: Mutex::new(None),
            travado: Mutex::new(None),
        })
    }

    fn mudar(&self, comportamento: Comportamento) {
        *self.comportamento.lock().unwrap() = comportamento;
    }

    fn eventos(&self) -> Vec<String> {
        self.eventos.lock().unwrap().clone()
    }

    fn criar_arquivos_parciais(&self) {
        fs::create_dir_all(&self.pasta_de_instalacao).unwrap();
        fs::write(self.pasta_de_instalacao.join("parcial.bin"), b"meio").unwrap();
        fs::write(self.pasta_de_instalacao.join("unins000.exe"), b"").unwrap();
    }
}

impl ExecutorDoInstalador for ExecutorFalso {
    fn instalar(
        &self,
        instalador: &Path,
        cancelamento: &CancellationToken,
    ) -> Result<(), ErroDeExecucao> {
        self.eventos.lock().unwrap().push("instalar".into());
        let existe = instalador.is_file();
        let hash = fs::read(instalador)
            .map(|bytes| {
                Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
            .unwrap_or_default();
        *self.arquivo_executado.lock().unwrap() = Some((existe, hash));

        let comportamento = *self.comportamento.lock().unwrap();
        match comportamento {
            Comportamento::Sucesso => {
                self.criar_arquivos_parciais();
                fs::write(self.pasta_de_instalacao.join("ollama.exe"), b"MZ").unwrap();
                Ok(())
            }
            Comportamento::SucessoSemOllama => Ok(()),
            Comportamento::FalhaNoMeio(codigo) => {
                self.criar_arquivos_parciais();
                Err(ErroDeExecucao::SaiuComCodigo(codigo))
            }
            Comportamento::FalhaComArquivoTravado => {
                self.criar_arquivos_parciais();
                // Abre sem permitir que outros apaguem, como um Ollama aberto faria.
                use std::os::windows::fs::OpenOptionsExt;
                let arquivo = fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(self.pasta_de_instalacao.join("parcial.bin"))
                    .unwrap();
                *self.travado.lock().unwrap() = Some(arquivo);
                Err(ErroDeExecucao::SaiuComCodigo(4))
            }
            Comportamento::ExigeElevacao => Err(ErroDeExecucao::ElevacaoNecessaria),
            Comportamento::FicaRodando => {
                self.criar_arquivos_parciais();
                while !cancelamento.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(ErroDeExecucao::Cancelado)
            }
        }
    }

    fn desinstalar(&self, desinstalador: &Path) -> io::Result<()> {
        self.eventos.lock().unwrap().push("desinstalar".into());
        if self.desinstalador_limpa {
            let _ = fs::remove_dir_all(desinstalador.parent().unwrap());
        }
        Ok(())
    }
}

// ----- Montagem do cenário -----

/// Conteúdo "do instalador": um pouco mais de 200 KB, determinístico.
fn conteudo_do_instalador() -> Vec<u8> {
    (0..210_000u32).map(|i| (i % 251) as u8).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn json_da_release(url_base: &str, tamanho: usize, sha256: &str) -> String {
    format!(
        r#"{{"tag_name":"v0.40.15","assets":[
            {{"name":"ollama-windows-amd64.zip","size":10,"digest":"sha256:{zeros}",
              "browser_download_url":"{url_base}/download/ollama-windows-amd64.zip"}},
            {{"name":"OllamaSetup.exe","size":{tamanho},"digest":"sha256:{sha256}",
              "browser_download_url":"{url_base}/download/OllamaSetup.exe"}}
        ]}}"#,
        zeros = "0".repeat(64)
    )
}

fn pedacos(conteudo: &[u8]) -> Vec<Passo> {
    let mut passos: Vec<Passo> = conteudo
        .chunks(50_000)
        .map(|p| Passo::Bytes(p.to_vec()))
        .collect();
    passos.push(Passo::Terminar);
    passos
}

struct Cenario {
    servidor: OllamaFalso,
    pasta: PastaTemporaria,
    executor: Arc<ExecutorFalso>,
    assinatura: Arc<AssinaturaFalsa>,
    instalador: InstaladorDoOllama,
}

struct Opcoes {
    espaco_livre: u64,
    assinatura: Result<(), FalhaDeAssinatura>,
    comportamento: Comportamento,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            espaco_livre: 100 * 1024 * 1024 * 1024,
            assinatura: Ok(()),
            comportamento: Comportamento::Sucesso,
        }
    }
}

/// Monta o cenário com um instalador que baixa normalmente.
async fn cenario(opcoes: Opcoes) -> Cenario {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(pedacos(&conteudo)),
    );
    montar(servidor, opcoes)
}

fn montar(servidor: OllamaFalso, opcoes: Opcoes) -> Cenario {
    let pasta = PastaTemporaria::nova();
    let config = ConfigInstalador {
        url_da_release: format!("{}/repos/ollama/ollama/releases/latest", servidor.url()),
        regras_de_origem: RegrasDeOrigem {
            hosts_permitidos: vec!["127.0.0.1".into()],
            exigir_https: false,
        },
        pasta_temporaria: pasta.download(),
        pasta_de_instalacao: pasta.instalacao(),
        publicador_esperado: "Ollama Inc.".into(),
        prazo_de_consulta: Duration::from_secs(6),
        ociosidade_do_download: Duration::from_millis(500),
    };
    let executor = ExecutorFalso::novo(pasta.instalacao(), opcoes.comportamento);
    let assinatura = Arc::new(AssinaturaFalsa {
        resultado: opcoes.assinatura,
        chamadas: Mutex::new(Vec::new()),
    });
    let http = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(4))
        .build()
        .unwrap();
    let instalador = InstaladorDoOllama::com_componentes(
        config,
        http,
        Arc::new(DetectorNaPasta(pasta.instalacao())),
        Arc::new(EspacoFalso(opcoes.espaco_livre)),
        Arc::clone(&assinatura) as Arc<dyn ValidadorDeAssinatura>,
        Arc::clone(&executor) as Arc<dyn ExecutorDoInstalador>,
    );
    Cenario {
        servidor,
        pasta,
        executor,
        assinatura,
        instalador,
    }
}

async fn instalar(
    cenario: &Cenario,
) -> (
    Result<ResultadoInstalacao, memoteca_lib::instalador::FalhaInstalacao>,
    Vec<ProgressoInstalacao>,
) {
    let mut progressos = Vec::new();
    let resultado = cenario
        .instalador
        .instalar(&CancellationToken::new(), &mut |p| progressos.push(p))
        .await;
    (resultado, progressos)
}

fn chamadas_ao_servidor(cenario: &Cenario) -> Vec<String> {
    cenario
        .servidor
        .requisicoes()
        .into_iter()
        .map(|r| r.caminho)
        .collect()
}

// ----- Caminho feliz -----

#[tokio::test]
async fn a_instalacao_termina_e_o_ollama_passa_a_ser_detectado() {
    let cenario = cenario(Opcoes::default()).await;

    let (resultado, progressos) = instalar(&cenario).await;

    let ResultadoInstalacao::Concluida { executavel } = resultado.expect("deveria instalar") else {
        panic!("deveria concluir");
    };
    assert_eq!(executavel, cenario.pasta.instalacao().join("ollama.exe"));
    assert!(executavel.is_file());

    // O instalador foi executado depois de baixado, íntegro e validado.
    let (existia, hash) = cenario
        .executor
        .arquivo_executado
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    assert!(existia);
    assert_eq!(hash, sha256_hex(&conteudo_do_instalador()));
    let validadas = cenario.assinatura.chamadas.lock().unwrap().clone();
    assert_eq!(validadas.len(), 1);
    assert!(
        validadas[0].1,
        "a assinatura é conferida no arquivo final .exe"
    );

    // O progresso passa pelas fases, na ordem.
    let fases: Vec<Fase> = progressos.iter().map(|p| p.fase).collect();
    let primeira = |fase| fases.iter().position(|f| *f == fase).unwrap();
    assert_eq!(fases.first(), Some(&Fase::Consultando));
    assert!(primeira(Fase::Consultando) < primeira(Fase::VerificandoEspaco));
    assert!(primeira(Fase::VerificandoEspaco) < primeira(Fase::Baixando));
    assert!(primeira(Fase::Baixando) < primeira(Fase::Verificando));
    assert!(primeira(Fase::Verificando) < primeira(Fase::Instalando));
    assert_eq!(fases.last(), Some(&Fase::Concluida));
    let ultimo_download = progressos
        .iter()
        .rfind(|p| p.fase == Fase::Baixando)
        .unwrap();
    assert_eq!(
        ultimo_download.baixado_em_bytes,
        ultimo_download.total_em_bytes
    );
    assert_eq!(
        ultimo_download.total_em_bytes,
        conteudo_do_instalador().len() as u64
    );
}

#[tokio::test]
async fn r14_o_instalador_baixado_e_removido_no_sucesso() {
    let cenario = cenario(Opcoes::default()).await;

    let (resultado, _) = instalar(&cenario).await;

    assert!(resultado.is_ok());
    assert!(
        cenario.pasta.arquivos_baixados().is_empty(),
        "sobrou: {:?}",
        cenario.pasta.arquivos_baixados()
    );
}

#[tokio::test]
async fn com_o_ollama_ja_instalado_nada_e_baixado_nem_alterado() {
    let cenario = cenario(Opcoes::default()).await;
    fs::create_dir_all(cenario.pasta.instalacao()).unwrap();
    fs::write(cenario.pasta.instalacao().join("ollama.exe"), b"do usuario").unwrap();

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroInstalacao::JaInstalado);
    assert!(
        chamadas_ao_servidor(&cenario).is_empty(),
        "nenhuma consulta ao servidor"
    );
    assert!(cenario.executor.eventos().is_empty());
    assert_eq!(
        fs::read(cenario.pasta.instalacao().join("ollama.exe")).unwrap(),
        b"do usuario"
    );
}

// ----- Espaço em disco (R08) -----

#[tokio::test]
async fn r08_sem_espaco_em_disco_a_instalacao_e_bloqueada_antes_de_baixar() {
    let cenario = cenario(Opcoes {
        espaco_livre: ESPACO_PARA_INSTALAR_EM_BYTES, // cabe a instalação, mas não o instalador também
        ..Opcoes::default()
    })
    .await;

    let (resultado, progressos) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    let ErroInstalacao::SemEspaco {
        necessario_em_bytes,
        disponivel_em_bytes,
    } = falha.erro
    else {
        panic!("deveria faltar espaço: {falha:?}");
    };
    assert_eq!(
        necessario_em_bytes,
        ESPACO_PARA_INSTALAR_EM_BYTES + conteudo_do_instalador().len() as u64
    );
    assert_eq!(disponivel_em_bytes, ESPACO_PARA_INSTALAR_EM_BYTES);
    assert!(!falha.restos_na_instalacao);
    assert!(falha.mensagem_amigavel().contains("espaço livre"));

    // Bloqueou antes de baixar: só a consulta da release foi feita.
    assert_eq!(
        chamadas_ao_servidor(&cenario),
        ["/repos/ollama/ollama/releases/latest"]
    );
    assert!(cenario.executor.eventos().is_empty());
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(progressos.iter().all(|p| p.fase != Fase::Baixando));
}

// ----- Sem internet e download interrompido (R11) -----

#[tokio::test]
async fn r11_sem_internet_o_erro_e_amigavel_e_nada_fica_pela_metade() {
    let servidor = OllamaFalso::iniciar().await;
    let porta = servidor.url();
    drop(servidor);
    // O endereço agora não tem ninguém ouvindo: conexão recusada.
    let cenario = {
        let servidor = OllamaFalso::iniciar().await;
        let mut cenario = montar(servidor, Opcoes::default());
        let pasta = &cenario.pasta;
        let config = ConfigInstalador {
            url_da_release: format!("{porta}/repos/ollama/ollama/releases/latest"),
            regras_de_origem: RegrasDeOrigem {
                hosts_permitidos: vec!["127.0.0.1".into()],
                exigir_https: false,
            },
            pasta_temporaria: pasta.download(),
            pasta_de_instalacao: pasta.instalacao(),
            publicador_esperado: "Ollama Inc.".into(),
            prazo_de_consulta: Duration::from_secs(6),
            ociosidade_do_download: Duration::from_millis(500),
        };
        cenario.instalador = InstaladorDoOllama::com_componentes(
            config,
            reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(Duration::from_secs(4))
                .build()
                .unwrap(),
            Arc::new(DetectorNaPasta(pasta.instalacao())),
            Arc::new(EspacoFalso(u64::MAX)),
            Arc::clone(&cenario.assinatura) as Arc<dyn ValidadorDeAssinatura>,
            Arc::clone(&cenario.executor) as Arc<dyn ExecutorDoInstalador>,
        );
        cenario
    };

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroInstalacao::SemInternet);
    assert!(falha.mensagem_amigavel().contains("conexão com a internet"));
    assert!(cenario.executor.eventos().is_empty());
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(!cenario.pasta.instalacao().exists());
}

#[tokio::test]
async fn r11_download_interrompido_no_meio_remove_o_arquivo_parcial_e_nao_instala() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(vec![
            Passo::Bytes(conteudo[..60_000].to_vec()),
            Passo::Cortar,
        ]),
    );
    let cenario = montar(servidor, Opcoes::default());

    let (resultado, _) = instalar(&cenario).await;

    assert_eq!(
        resultado.unwrap_err().erro,
        ErroInstalacao::DownloadInterrompido
    );
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(cenario.executor.eventos().is_empty());
}

#[tokio::test]
async fn download_que_termina_antes_do_tamanho_informado_e_interrompido() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(pedacos(&conteudo[..100_000])),
    );
    let cenario = montar(servidor, Opcoes::default());

    let (resultado, _) = instalar(&cenario).await;

    assert_eq!(
        resultado.unwrap_err().erro,
        ErroInstalacao::DownloadInterrompido
    );
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(cenario.executor.eventos().is_empty());
}

#[tokio::test]
async fn servidor_de_download_travado_estoura_o_tempo_sem_instalar() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(vec![
            Passo::Bytes(conteudo[..1000].to_vec()),
            Passo::Silencio,
        ]),
    );
    let cenario = montar(servidor, Opcoes::default());

    let (resultado, _) = instalar(&cenario).await;

    assert_eq!(resultado.unwrap_err().erro, ErroInstalacao::TempoEsgotado);
    assert!(cenario.pasta.arquivos_baixados().is_empty());
}

#[tokio::test]
async fn erro_http_na_consulta_ou_no_download_vira_servidor_indisponivel() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::erro(403, "rate limit"),
    );
    let cenario = montar(servidor, Opcoes::default());
    assert_eq!(
        instalar(&cenario).await.0.unwrap_err().erro,
        ErroInstalacao::ServidorIndisponivel
    );

    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::erro(500, "falha"),
    );
    let cenario = montar(servidor, Opcoes::default());
    let falha = instalar(&cenario).await.0.unwrap_err();
    assert_eq!(falha.erro, ErroInstalacao::ServidorIndisponivel);
    assert!(!falha.mensagem_amigavel().contains("falha"));
}

// ----- Validação do arquivo baixado -----

#[tokio::test]
async fn r13_arquivo_com_hash_diferente_nao_e_executado_e_e_removido() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    let mut adulterado = conteudo.clone();
    adulterado[1234] ^= 0xFF; // mesmo tamanho, um byte diferente
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(pedacos(&adulterado)),
    );
    let cenario = montar(servidor, Opcoes::default());

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert!(matches!(
        falha.erro,
        ErroInstalacao::InstaladorCorrompido(_)
    ));
    assert!(falha.mensagem_amigavel().contains("danificado"));
    assert!(cenario.executor.eventos().is_empty(), "não pode executar");
    assert!(cenario.assinatura.chamadas.lock().unwrap().is_empty());
    assert!(cenario.pasta.arquivos_baixados().is_empty());
}

#[tokio::test]
async fn r13_arquivo_maior_que_o_informado_e_recusado() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    let mut maior = conteudo.clone();
    maior.extend_from_slice(&[1; 5000]);
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(pedacos(&maior)),
    );
    let cenario = montar(servidor, Opcoes::default());

    let (resultado, _) = instalar(&cenario).await;

    assert!(matches!(
        resultado.unwrap_err().erro,
        ErroInstalacao::InstaladorCorrompido(_)
    ));
    assert!(cenario.executor.eventos().is_empty());
    assert!(cenario.pasta.arquivos_baixados().is_empty());
}

#[tokio::test]
async fn r13_assinatura_invalida_ou_de_outro_publicador_impede_a_execucao() {
    let falhas = [
        FalhaDeAssinatura::NaoAssinado,
        FalhaDeAssinatura::Invalida("HashMismatch".into()),
        FalhaDeAssinatura::PublicadorDiferente("CN=Outra Empresa".into()),
        FalhaDeAssinatura::NaoFoiPossivelVerificar("powershell travou".into()),
    ];
    for assinatura in falhas {
        let cenario = cenario(Opcoes {
            assinatura: Err(assinatura.clone()),
            ..Opcoes::default()
        })
        .await;

        let (resultado, _) = instalar(&cenario).await;

        let falha = resultado.unwrap_err();
        assert!(
            matches!(falha.erro, ErroInstalacao::AssinaturaInvalida(_)),
            "{assinatura:?}"
        );
        assert!(falha.mensagem_amigavel().contains("não foi executado"));
        assert!(cenario.executor.eventos().is_empty(), "{assinatura:?}");
        assert!(
            cenario.pasta.arquivos_baixados().is_empty(),
            "{assinatura:?}"
        );
        assert!(!cenario.pasta.instalacao().exists());
    }
}

#[tokio::test]
async fn release_sem_instalador_sem_hash_ou_de_origem_nao_permitida_e_recusada() {
    let conteudo = conteudo_do_instalador();
    let sha = sha256_hex(&conteudo);
    let casos: Vec<(&str, String)> = vec![
        ("sem o instalador", r#"{"tag_name":"v1","assets":[]}"#.to_owned()),
        (
            "sem hash",
            r#"{"tag_name":"v1","assets":[{"name":"OllamaSetup.exe","size":10,"browser_download_url":"http://127.0.0.1:1/x"}]}"#.to_owned(),
        ),
        (
            "hash de outro tipo",
            r#"{"tag_name":"v1","assets":[{"name":"OllamaSetup.exe","size":10,"digest":"md5:abc","browser_download_url":"http://127.0.0.1:1/x"}]}"#.to_owned(),
        ),
        (
            "host não permitido",
            format!(
                r#"{{"tag_name":"v1","assets":[{{"name":"OllamaSetup.exe","size":10,"digest":"sha256:{sha}","browser_download_url":"http://evil.example/OllamaSetup.exe"}}]}}"#
            ),
        ),
        ("JSON inválido", "isto não é json".to_owned()),
    ];

    for (descricao, corpo) in casos {
        let servidor = OllamaFalso::iniciar().await;
        servidor.rota(
            "GET",
            "/repos/ollama/ollama/releases/latest",
            Resposta::json(&corpo),
        );
        let cenario = montar(servidor, Opcoes::default());

        let (resultado, _) = instalar(&cenario).await;

        let falha = resultado.unwrap_err();
        assert!(
            matches!(falha.erro, ErroInstalacao::ReleaseInvalida(_)),
            "{descricao}: {falha:?}"
        );
        assert_eq!(
            chamadas_ao_servidor(&cenario),
            ["/repos/ollama/ollama/releases/latest"],
            "{descricao}: não pode baixar nada"
        );
    }
}

// ----- Instalação parcial e rollback (R12, R15) -----

#[tokio::test]
async fn r12_falha_no_meio_da_instalacao_desfaz_tudo_e_permite_tentar_de_novo() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::FalhaNoMeio(4),
        ..Opcoes::default()
    })
    .await;

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroInstalacao::InstaladorFalhou(4));
    assert!(!falha.restos_na_instalacao);
    let mensagem = falha.mensagem_amigavel();
    assert!(mensagem.contains("não foi concluída"));
    assert!(!mensagem.contains('4'), "sem código de saída na tela");
    // Rollback: o desinstalador rodou e a pasta sumiu. O instalador baixado também.
    assert_eq!(cenario.executor.eventos(), ["instalar", "desinstalar"]);
    assert!(!cenario.pasta.instalacao().exists(), "sem resíduo do app");
    assert!(cenario.pasta.arquivos_baixados().is_empty());

    // "Tentar de novo": agora o instalador funciona e a segunda tentativa dá certo.
    cenario.executor.mudar(Comportamento::Sucesso);
    let (segunda, _) = instalar(&cenario).await;
    assert!(matches!(segunda, Ok(ResultadoInstalacao::Concluida { .. })));
}

#[tokio::test]
async fn r12_se_o_desinstalador_nao_limpa_o_app_remove_a_pasta_que_ele_mesmo_criou() {
    let cenario = {
        let mut c = cenario(Opcoes {
            comportamento: Comportamento::FalhaNoMeio(3),
            ..Opcoes::default()
        })
        .await;
        // Troca o executor por um cujo desinstalador não faz nada.
        let executor = Arc::new(ExecutorFalso {
            pasta_de_instalacao: c.pasta.instalacao(),
            comportamento: Mutex::new(Comportamento::FalhaNoMeio(3)),
            desinstalador_limpa: false,
            eventos: Mutex::new(Vec::new()),
            arquivo_executado: Mutex::new(None),
            travado: Mutex::new(None),
        });
        c.instalador = reconstruir(&c, Arc::clone(&executor) as Arc<dyn ExecutorDoInstalador>);
        c.executor = executor;
        c
    };

    let (resultado, _) = instalar(&cenario).await;

    assert!(!resultado.unwrap_err().restos_na_instalacao);
    assert!(
        cenario
            .executor
            .eventos()
            .contains(&"desinstalar".to_owned())
    );
    assert!(!cenario.pasta.instalacao().exists());
}

/// Recria o instalador de um cenário trocando só o executor.
fn reconstruir(cenario: &Cenario, executor: Arc<dyn ExecutorDoInstalador>) -> InstaladorDoOllama {
    let config = ConfigInstalador {
        url_da_release: format!(
            "{}/repos/ollama/ollama/releases/latest",
            cenario.servidor.url()
        ),
        regras_de_origem: RegrasDeOrigem {
            hosts_permitidos: vec!["127.0.0.1".into()],
            exigir_https: false,
        },
        pasta_temporaria: cenario.pasta.download(),
        pasta_de_instalacao: cenario.pasta.instalacao(),
        publicador_esperado: "Ollama Inc.".into(),
        prazo_de_consulta: Duration::from_secs(6),
        ociosidade_do_download: Duration::from_millis(500),
    };
    InstaladorDoOllama::com_componentes(
        config,
        reqwest::Client::builder().no_proxy().build().unwrap(),
        Arc::new(DetectorNaPasta(cenario.pasta.instalacao())),
        Arc::new(EspacoFalso(u64::MAX)),
        Arc::clone(&cenario.assinatura) as Arc<dyn ValidadorDeAssinatura>,
        executor,
    )
}

#[tokio::test]
async fn r12_restos_que_nao_podem_ser_removidos_sao_informados_na_mensagem() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::FalhaComArquivoTravado,
        ..Opcoes::default()
    })
    .await;
    // O desinstalador do cenário padrão limparia a pasta, mas o arquivo travado impede.

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert!(
        falha.restos_na_instalacao,
        "o arquivo travado impede a remoção"
    );
    assert!(
        falha
            .mensagem_amigavel()
            .contains("não puderam ser removidos")
    );
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    // Libera o arquivo para a pasta de teste poder ser apagada.
    cenario.executor.travado.lock().unwrap().take();
}

#[tokio::test]
async fn o_rollback_nao_mexe_numa_pasta_que_ja_existia_antes_da_tentativa() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::FalhaNoMeio(4),
        ..Opcoes::default()
    })
    .await;
    fs::create_dir_all(cenario.pasta.instalacao()).unwrap();
    fs::write(
        cenario.pasta.instalacao().join("meus-dados.txt"),
        "do usuário",
    )
    .unwrap();

    let (resultado, _) = instalar(&cenario).await;

    assert!(resultado.is_err());
    assert_eq!(
        fs::read_to_string(cenario.pasta.instalacao().join("meus-dados.txt")).unwrap(),
        "do usuário"
    );
    assert!(
        !cenario
            .executor
            .eventos()
            .contains(&"desinstalar".to_owned()),
        "não é o Memoteca quem deve limpar uma pasta que já existia"
    );
}

#[tokio::test]
async fn instalador_que_termina_bem_mas_nao_deixa_o_ollama_e_desfeito() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::SucessoSemOllama,
        ..Opcoes::default()
    })
    .await;

    let (resultado, _) = instalar(&cenario).await;

    assert_eq!(
        resultado.unwrap_err().erro,
        ErroInstalacao::InstalacaoNaoDetectada
    );
    assert!(cenario.pasta.arquivos_baixados().is_empty());
}

#[tokio::test]
async fn r15_se_o_windows_exigir_elevacao_o_erro_e_amigavel_e_nada_fica() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::ExigeElevacao,
        ..Opcoes::default()
    })
    .await;

    let (resultado, _) = instalar(&cenario).await;

    let falha = resultado.unwrap_err();
    assert_eq!(falha.erro, ErroInstalacao::ElevacaoNecessaria);
    assert!(falha.mensagem_amigavel().contains("administrador"));
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(!cenario.pasta.instalacao().exists());
}

// ----- Cancelamento -----

#[tokio::test]
async fn cancelar_durante_o_download_remove_o_arquivo_e_nao_instala() {
    let conteudo = conteudo_do_instalador();
    let servidor = OllamaFalso::iniciar().await;
    servidor.rota(
        "GET",
        "/repos/ollama/ollama/releases/latest",
        Resposta::json(&json_da_release(
            &servidor.url(),
            conteudo.len(),
            &sha256_hex(&conteudo),
        )),
    );
    servidor.rota(
        "GET",
        "/download/OllamaSetup.exe",
        Resposta::Stream(vec![
            Passo::Bytes(conteudo[..5000].to_vec()),
            Passo::Silencio,
        ]),
    );
    let cenario = montar(servidor, Opcoes::default());
    let cancelamento = CancellationToken::new();
    let para_cancelar = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        para_cancelar.cancel();
    });

    let resultado = cenario
        .instalador
        .instalar(&cancelamento, &mut |_| {})
        .await
        .expect("cancelar não é erro");

    assert_eq!(resultado, ResultadoInstalacao::Cancelada);
    assert!(cenario.pasta.arquivos_baixados().is_empty());
    assert!(cenario.executor.eventos().is_empty());
}

#[tokio::test]
async fn cancelar_durante_a_instalacao_encerra_o_instalador_e_desfaz_o_que_ficou() {
    let cenario = cenario(Opcoes {
        comportamento: Comportamento::FicaRodando,
        ..Opcoes::default()
    })
    .await;
    let cancelamento = CancellationToken::new();
    let para_cancelar = cancelamento.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        para_cancelar.cancel();
    });

    let resultado = cenario
        .instalador
        .instalar(&cancelamento, &mut |_| {})
        .await
        .expect("cancelar não é erro");

    assert_eq!(resultado, ResultadoInstalacao::Cancelada);
    assert_eq!(cenario.executor.eventos(), ["instalar", "desinstalar"]);
    assert!(!cenario.pasta.instalacao().exists());
    assert!(cenario.pasta.arquivos_baixados().is_empty());
}

// ----- Mensagens -----

#[tokio::test]
async fn nenhuma_mensagem_amigavel_vaza_texto_tecnico() {
    let erros = [
        ErroInstalacao::ReleaseInvalida("http://evil.example/x".into()),
        ErroInstalacao::InstaladorCorrompido("sha256 abc".into()),
        ErroInstalacao::AssinaturaInvalida("CN=Outra, O=Empresa".into()),
        ErroInstalacao::InstaladorFalhou(4),
        ErroInstalacao::NaoFoiPossivelExecutar("C:\\Users\\x\\OllamaSetup.exe".into()),
        ErroInstalacao::Interno("os error 5".into()),
    ];
    for erro in erros {
        let mensagem = erro.mensagem_amigavel();
        for proibido in ["http", "sha256", "CN=", "C:\\", "os error", ".exe"] {
            assert!(
                !mensagem.contains(proibido),
                "`{mensagem}` contém {proibido}"
            );
        }
    }
}

// ----- Assinatura real, pelo PowerShell do Windows -----

#[cfg(windows)]
mod assinatura_real {
    use memoteca_lib::instalador::assinatura::{
        FalhaDeAssinatura, ValidadorDeAssinatura, ValidadorPowerShell,
    };
    use std::path::Path;

    #[test]
    fn aceita_um_arquivo_do_windows_com_o_publicador_certo() {
        let kernel32 = Path::new(r"C:\Windows\System32\kernel32.dll");

        let resultado = ValidadorPowerShell.validar(kernel32, "Microsoft Corporation");

        assert_eq!(resultado, Ok(()));
    }

    #[test]
    fn recusa_um_arquivo_assinado_por_outro_publicador() {
        let kernel32 = Path::new(r"C:\Windows\System32\kernel32.dll");

        let resultado = ValidadorPowerShell.validar(kernel32, "Ollama Inc.");

        assert!(matches!(
            resultado,
            Err(FalhaDeAssinatura::PublicadorDiferente(_))
        ));
    }

    #[test]
    fn recusa_um_arquivo_sem_assinatura() {
        let proprio = std::env::current_exe().unwrap();

        let resultado = ValidadorPowerShell.validar(&proprio, "Ollama Inc.");

        assert_eq!(resultado, Err(FalhaDeAssinatura::NaoAssinado));
    }

    #[test]
    fn um_caminho_com_caracteres_especiais_nao_e_interpretado_como_comando() {
        let pasta = std::env::temp_dir().join("memoteca assinatura '$(calc)' ; & teste");
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("arquivo.exe");
        std::fs::write(&arquivo, b"nao e um executavel").unwrap();

        let resultado = ValidadorPowerShell.validar(&arquivo, "Ollama Inc.");

        let _ = std::fs::remove_dir_all(&pasta);
        // Um arquivo que não é um executável não tem assinatura válida. O que importa aqui é
        // que o caminho foi tratado como texto, e não executado.
        assert!(resultado.is_err(), "{resultado:?}");
    }
}
