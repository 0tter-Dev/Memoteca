//! Origem do instalador: consulta da release oficial e download com SHA-256.
//!
//! A release mais recente do Ollama no GitHub informa, para cada arquivo, o tamanho e o
//! SHA-256 (`digest`). O Memoteca baixa o `OllamaSetup.exe` da release, calcula o SHA-256
//! enquanto baixa e só entrega o arquivo se tamanho e hash baterem.

use std::path::Path;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use super::erro::ErroInstalacao;

/// Nome do instalador dentro da release.
pub const NOME_DO_INSTALADOR: &str = "OllamaSetup.exe";

/// De onde o instalador pode vir.
#[derive(Debug, Clone)]
pub struct RegrasDeOrigem {
    /// Hosts aceitos para a URL de download informada pela release.
    pub hosts_permitidos: Vec<String>,
    /// Exige `https`. Só os testes, que falam com `127.0.0.1`, desligam isto.
    pub exigir_https: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDoInstalador {
    pub versao: String,
    pub url: String,
    pub tamanho_em_bytes: u64,
    /// SHA-256 em hexadecimal minúsculo.
    pub sha256: String,
}

#[derive(Deserialize)]
struct ReleaseBruta {
    tag_name: Option<String>,
    assets: Vec<AssetBruto>,
}

#[derive(Deserialize)]
struct AssetBruto {
    name: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}

/// Lê um `digest` no formato `sha256:<64 dígitos hexadecimais>`.
pub fn ler_digest(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

fn validar_origem(url: &str, regras: &RegrasDeOrigem) -> Result<(), ErroInstalacao> {
    let invalida = |motivo: &str| ErroInstalacao::ReleaseInvalida(format!("{motivo}: {url}"));
    let analisada = reqwest::Url::parse(url).map_err(|_| invalida("URL inválida"))?;

    if regras.exigir_https && analisada.scheme() != "https" {
        return Err(invalida("a URL de download não usa https"));
    }
    let host = analisada.host_str().unwrap_or_default();
    if !regras
        .hosts_permitidos
        .iter()
        .any(|permitido| permitido == host)
    {
        return Err(invalida("host de download não permitido"));
    }
    Ok(())
}

/// Traduz uma falha de rede antes de a resposta chegar.
fn erro_de_rede(erro: &reqwest::Error) -> ErroInstalacao {
    if erro.is_connect() || erro.is_timeout() || erro.is_request() {
        ErroInstalacao::SemInternet
    } else {
        ErroInstalacao::ServidorIndisponivel
    }
}

/// Consulta a release e escolhe o instalador.
pub async fn consultar_release(
    http: &reqwest::Client,
    url_da_api: &str,
    prazo: Duration,
    regras: &RegrasDeOrigem,
) -> Result<ReleaseDoInstalador, ErroInstalacao> {
    let resposta = http
        .get(url_da_api)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .timeout(prazo)
        .send()
        .await
        .map_err(|erro| erro_de_rede(&erro))?;
    if !resposta.status().is_success() {
        return Err(ErroInstalacao::ServidorIndisponivel);
    }
    let bytes = resposta.bytes().await.map_err(|erro| erro_de_rede(&erro))?;

    let release: ReleaseBruta = serde_json::from_slice(&bytes)
        .map_err(|erro| ErroInstalacao::ReleaseInvalida(erro.to_string()))?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == NOME_DO_INSTALADOR)
        .ok_or_else(|| {
            ErroInstalacao::ReleaseInvalida(format!("a release não tem o {NOME_DO_INSTALADOR}"))
        })?;

    let sha256 = asset
        .digest
        .as_deref()
        .and_then(ler_digest)
        .ok_or_else(|| ErroInstalacao::ReleaseInvalida("a release não informa o SHA-256".into()))?;
    if asset.size == 0 {
        return Err(ErroInstalacao::ReleaseInvalida(
            "a release informa tamanho zero".into(),
        ));
    }
    validar_origem(&asset.browser_download_url, regras)?;

    Ok(ReleaseDoInstalador {
        versao: release.tag_name.unwrap_or_default(),
        url: asset.browser_download_url,
        tamanho_em_bytes: asset.size,
        sha256,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Baixado {
    /// Todos os bytes chegaram, com este SHA-256 (hexadecimal minúsculo).
    Completo {
        sha256: String,
    },
    Cancelado,
}

/// Baixa o instalador para `destino`. `ao_progredir` recebe `(baixado, total)` com no
/// máximo uma chamada a cada 200 ms (mais a última). Cancelar fecha a conexão.
pub async fn baixar(
    http: &reqwest::Client,
    release: &ReleaseDoInstalador,
    destino: &Path,
    ociosidade: Duration,
    cancelamento: &CancellationToken,
    ao_progredir: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<Baixado, ErroInstalacao> {
    let envio = http.get(&release.url).send();
    let resposta = tokio::select! {
        biased;
        () = cancelamento.cancelled() => return Ok(Baixado::Cancelado),
        resultado = tokio::time::timeout(ociosidade, envio) => resultado,
    };
    let resposta = match resposta {
        Err(_) => return Err(ErroInstalacao::TempoEsgotado),
        Ok(Err(erro)) => return Err(erro_de_rede(&erro)),
        Ok(Ok(resposta)) => resposta,
    };
    if !resposta.status().is_success() {
        return Err(ErroInstalacao::ServidorIndisponivel);
    }
    if let Some(informado) = resposta.content_length()
        && informado != release.tamanho_em_bytes
    {
        return Err(ErroInstalacao::InstaladorCorrompido(format!(
            "o servidor informa {informado} bytes, e a release {}",
            release.tamanho_em_bytes
        )));
    }

    let mut arquivo = tokio::fs::File::create(destino)
        .await
        .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;
    let fluxo = resposta.bytes_stream();
    tokio::pin!(fluxo);

    let mut hash = Sha256::new();
    let mut baixado: u64 = 0;
    let mut ultimo_aviso = Instant::now();
    loop {
        let proximo = tokio::select! {
            biased;
            () = cancelamento.cancelled() => return Ok(Baixado::Cancelado),
            resultado = tokio::time::timeout(ociosidade, fluxo.next()) => resultado,
        };
        let pedaco = match proximo {
            Err(_) => return Err(ErroInstalacao::TempoEsgotado),
            Ok(None) => break,
            Ok(Some(Err(erro))) => {
                return Err(if erro.is_timeout() {
                    ErroInstalacao::TempoEsgotado
                } else {
                    ErroInstalacao::DownloadInterrompido
                });
            }
            Ok(Some(Ok(pedaco))) => pedaco,
        };

        baixado += pedaco.len() as u64;
        if baixado > release.tamanho_em_bytes {
            return Err(ErroInstalacao::InstaladorCorrompido(
                "o arquivo é maior que o informado pela release".into(),
            ));
        }
        hash.update(&pedaco);
        arquivo
            .write_all(&pedaco)
            .await
            .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;

        if ultimo_aviso.elapsed() >= Duration::from_millis(200) {
            ultimo_aviso = Instant::now();
            ao_progredir(baixado, release.tamanho_em_bytes);
        }
    }

    arquivo
        .flush()
        .await
        .map_err(|erro| ErroInstalacao::Interno(erro.to_string()))?;
    drop(arquivo);
    ao_progredir(baixado, release.tamanho_em_bytes);

    if baixado != release.tamanho_em_bytes {
        // A conexão acabou antes do fim sem dar erro: arquivo truncado.
        return Err(ErroInstalacao::DownloadInterrompido);
    }
    let sha256 = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(Baixado::Completo { sha256 })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_o_digest_da_release() {
        let hex = "106ef3d2a02437810fba6f574914b1413c512ea84ddd4f184c8e7f434b6f5564";

        assert_eq!(ler_digest(&format!("sha256:{hex}")), Some(hex.to_owned()));
        assert_eq!(
            ler_digest(&format!("sha256:{}", hex.to_uppercase())),
            Some(hex.to_owned())
        );
    }

    #[test]
    fn recusa_digest_de_outro_formato() {
        for digest in [
            "",
            "sha256:",
            "sha256:abc",
            "md5:106ef3d2a02437810fba6f574914b141",
            "106ef3d2a02437810fba6f574914b1413c512ea84ddd4f184c8e7f434b6f5564",
        ] {
            assert_eq!(ler_digest(digest), None, "{digest:?}");
        }
        let nao_hex = format!("sha256:{}", "z".repeat(64));
        assert_eq!(ler_digest(&nao_hex), None);
    }

    #[test]
    fn a_origem_exige_https_e_host_permitido() {
        let regras = RegrasDeOrigem {
            hosts_permitidos: vec!["github.com".into()],
            exigir_https: true,
        };

        assert!(
            validar_origem(
                "https://github.com/ollama/ollama/releases/download/v1/OllamaSetup.exe",
                &regras
            )
            .is_ok()
        );
        assert!(validar_origem("http://github.com/x", &regras).is_err());
        assert!(validar_origem("https://github.com.evil.example/x", &regras).is_err());
        assert!(validar_origem("https://evil.example/github.com/x", &regras).is_err());
        assert!(validar_origem("não é url", &regras).is_err());
    }
}
