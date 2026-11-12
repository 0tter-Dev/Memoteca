//! Peças que só existem no build dos testes ponta a ponta (E15), com `--features e2e`.
//!
//! O E2E roda o aplicativo de verdade, mas não pode tocar nos dados do usuário, abrir janelas
//! do Windows que o teste não alcança, instalar o Ollama de verdade nem depender do espaço do
//! disco. Este módulo troca só esses pontos, e **nunca entra no build de release**: o recurso
//! `e2e` não faz parte dos recursos padrão, e um teste (`tests/ci.rs`) confere isso.
//!
//! Variáveis de ambiente que o teste define ao iniciar o app:
//!
//! - `MEMOTECA_E2E_DADOS`: pasta de dados do app (banco, cópias, log) em vez da pasta do usuário;
//! - `MEMOTECA_E2E_ESCOLHAS`: pasta que substitui as janelas "Exportar" e "Importar";
//! - `MEMOTECA_E2E_CDP_PORT`: porta de depuração do WebView2, por onde o Playwright se conecta;
//! - `MEMOTECA_E2E_RELEASE_URL`: endereço da "release" do Ollama (servidor falso);
//! - `MEMOTECA_E2E_OLLAMA_FALSO`: o executável que o instalador falso "instala" como `ollama.exe`.
//!
//! O espaço livre do disco vem do arquivo `espaco-livre.txt` dentro da pasta de dados (em
//! bytes), lido a cada consulta, para o teste mudá-lo com o app aberto. Sem o arquivo, há
//! espaço de sobra.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::instalador::assinatura::{FalhaDeAssinatura, ValidadorDeAssinatura};
use crate::instalador::disco::EspacoEmDisco;
use crate::instalador::executor::{ErroDeExecucao, ExecutorDoInstalador};
use crate::instalador::fonte::RegrasDeOrigem;
use crate::instalador::{ConfigInstalador, ErroInstalacao, InstaladorDoOllama};
use crate::ollama::instalacao::DetectorDeInstalacao;

/// Pasta de dados do app no teste, se o teste a definiu.
pub fn pasta_de_dados() -> Option<PathBuf> {
    std::env::var_os("MEMOTECA_E2E_DADOS").map(PathBuf::from)
}

/// No lugar da janela do Windows: salvar grava em `<escolhas>/<nome sugerido>`, e abrir lê
/// `<escolhas>/importar.json`, se existir (senão, é como se o usuário tivesse cancelado).
pub fn escolher_arquivo(salvar_como: Option<String>) -> Option<PathBuf> {
    let pasta = PathBuf::from(std::env::var_os("MEMOTECA_E2E_ESCOLHAS")?);
    match salvar_como {
        Some(nome) => Some(pasta.join(nome)),
        None => {
            let arquivo = pasta.join("importar.json");
            arquivo.is_file().then_some(arquivo)
        }
    }
}

/// Espaço livre lido de `espaco-livre.txt`.
pub struct EspacoDoE2e;

impl EspacoEmDisco for EspacoDoE2e {
    fn livre_em(&self, _pasta: &Path) -> io::Result<u64> {
        let Some(pasta) = pasta_de_dados() else {
            return Ok(u64::MAX / 2);
        };
        match std::fs::read_to_string(pasta.join("espaco-livre.txt")) {
            Ok(texto) => texto
                .trim()
                .parse()
                .map_err(|_| io::Error::other("espaco-livre.txt inválido")),
            Err(_) => Ok(u64::MAX / 2),
        }
    }
}

/// A assinatura do instalador falso não existe: aqui ela é sempre aceita. Só vale para o
/// arquivo falso servido pelo servidor de teste.
struct AssinaturaAceita;

impl ValidadorDeAssinatura for AssinaturaAceita {
    fn validar(&self, _arquivo: &Path, _publicador: &str) -> Result<(), FalhaDeAssinatura> {
        Ok(())
    }
}

/// "Instala" copiando o Ollama falso para a pasta de instalação. Não executa o arquivo baixado.
struct ExecutorFalso {
    ollama_falso: PathBuf,
    pasta_de_instalacao: PathBuf,
}

impl ExecutorDoInstalador for ExecutorFalso {
    fn instalar(
        &self,
        _instalador: &Path,
        _cancelamento: &CancellationToken,
    ) -> Result<(), ErroDeExecucao> {
        std::fs::create_dir_all(&self.pasta_de_instalacao)
            .and_then(|()| {
                std::fs::copy(
                    &self.ollama_falso,
                    self.pasta_de_instalacao.join("ollama.exe"),
                )
            })
            .map(|_| ())
            .map_err(|erro| ErroDeExecucao::NaoIniciou(erro.to_string()))
    }

    fn desinstalar(&self, _desinstalador: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// O instalador do Ollama do build de teste: consulta e baixa do servidor falso, aceita a
/// assinatura e "instala" copiando o Ollama falso.
pub fn instalador(
    detector: Arc<dyn DetectorDeInstalacao>,
) -> Result<InstaladorDoOllama, ErroInstalacao> {
    let ausente = |nome: &str| ErroInstalacao::Interno(format!("falta a variável {nome}"));
    let mut config = ConfigInstalador::padrao()
        .ok_or_else(|| ErroInstalacao::Interno("sem pasta do usuário".into()))?;
    config.url_da_release = std::env::var("MEMOTECA_E2E_RELEASE_URL")
        .map_err(|_| ausente("MEMOTECA_E2E_RELEASE_URL"))?;
    config.regras_de_origem = RegrasDeOrigem {
        hosts_permitidos: vec!["127.0.0.1".to_owned()],
        exigir_https: false,
    };
    let ollama_falso = std::env::var_os("MEMOTECA_E2E_OLLAMA_FALSO")
        .map(PathBuf::from)
        .ok_or_else(|| ausente("MEMOTECA_E2E_OLLAMA_FALSO"))?;

    let http = reqwest::Client::builder()
        .connect_timeout(config.prazo_de_consulta)
        .build()
        .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;
    let executor = ExecutorFalso {
        ollama_falso,
        pasta_de_instalacao: config.pasta_de_instalacao.clone(),
    };
    Ok(InstaladorDoOllama::com_componentes(
        config,
        http,
        detector,
        Arc::new(EspacoDoE2e),
        Arc::new(AssinaturaAceita),
        Arc::new(executor),
    ))
}

/// Cria a janela principal no build de teste. No `e2e.tauri.conf.json` a lista de janelas é
/// vazia, e a janela é criada aqui, com a porta de depuração do WebView2 (CDP) que o teste pede
/// em `MEMOTECA_E2E_CDP_PORT`. A variável de ambiente do WebView2 não chega a ele em todo
/// ambiente (no runner do GitHub foi ignorada), e passar o argumento pela janela funciona sempre.
/// O título e os tamanhos são os do `tauri.conf.json`.
pub fn criar_janela(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let mut janela = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Memoteca")
        .inner_size(1100.0, 720.0)
        .min_inner_size(800.0, 560.0);
    #[cfg(windows)]
    if let Ok(porta) = std::env::var("MEMOTECA_E2E_CDP_PORT") {
        // Mantém os argumentos que o Tauri já passa ao WebView2.
        janela = janela.additional_browser_args(&format!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={porta}"
        ));
    }
    janela.build().map(|_| ())
}
