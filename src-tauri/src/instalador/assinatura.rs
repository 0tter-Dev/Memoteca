//! Validação da assinatura digital (Authenticode) do instalador.
//!
//! A leitura é feita pelo próprio Windows, com `Get-AuthenticodeSignature` do PowerShell, que
//! confere a assinatura e a cadeia de certificados (inclusive assinaturas por catálogo). O
//! Memoteca exige que o status seja `Valid` e que o publicador do certificado seja o esperado.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Publicador esperado nos binários oficiais do Ollama.
pub const PUBLICADOR_DO_OLLAMA: &str = "Ollama Inc.";

/// Por que a assinatura não foi aceita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalhaDeAssinatura {
    /// O arquivo não tem assinatura.
    NaoAssinado,
    /// A assinatura existe, mas não é válida (adulterada, certificado não confiável...).
    Invalida(String),
    /// A assinatura é válida, mas o publicador não é o esperado.
    PublicadorDiferente(String),
    /// Não foi possível consultar o Windows.
    NaoFoiPossivelVerificar(String),
}

/// Quem valida a assinatura. Os testes usam versões falsas.
pub trait ValidadorDeAssinatura: Send + Sync {
    /// Confere que `arquivo` tem assinatura válida de `publicador`. Bloqueia até terminar.
    fn validar(&self, arquivo: &Path, publicador: &str) -> Result<(), FalhaDeAssinatura>;
}

/// Valida com o PowerShell do Windows.
pub struct ValidadorPowerShell;

/// Tempo máximo para o Windows verificar. O arquivo inteiro é lido para calcular o hash.
const PRAZO_DA_VERIFICACAO: Duration = Duration::from_secs(300);

const SCRIPT: &str = "try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }; \
    $s = Get-AuthenticodeSignature -LiteralPath $env:MEMOTECA_ARQUIVO; \
    $assunto = ''; if ($s.SignerCertificate) { $assunto = $s.SignerCertificate.Subject }; \
    [pscustomobject]@{ Status = $s.Status.ToString(); Subject = $assunto } | ConvertTo-Json -Compress";

#[derive(Deserialize)]
struct SaidaDoPowerShell {
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Subject")]
    assunto: String,
}

impl ValidadorDeAssinatura for ValidadorPowerShell {
    fn validar(&self, arquivo: &Path, publicador: &str) -> Result<(), FalhaDeAssinatura> {
        let (saida, erros) = executar_powershell(arquivo)
            .map_err(|erro| FalhaDeAssinatura::NaoFoiPossivelVerificar(erro.to_string()))?;
        interpretar(&saida, publicador).map_err(|falha| match falha {
            // Sem saída útil, o que o PowerShell escreveu em stderr explica o motivo.
            FalhaDeAssinatura::NaoFoiPossivelVerificar(detalhe) => {
                let erros: String = erros.trim().chars().take(400).collect();
                FalhaDeAssinatura::NaoFoiPossivelVerificar(format!("{detalhe}; stderr: {erros}"))
            }
            outra => outra,
        })
    }
}

/// Lê um fluxo inteiro numa thread, para o prazo da verificação valer mesmo se travar.
fn ler_em_thread<R: io::Read + Send + 'static>(
    mut fluxo: R,
) -> std::thread::JoinHandle<io::Result<String>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        fluxo.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
}

/// Devolve a saída padrão e a de erros do PowerShell.
fn executar_powershell(arquivo: &Path) -> io::Result<(String, String)> {
    // Caminho completo do sistema, para não depender do `PATH`.
    let raiz = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let powershell = Path::new(&raiz)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");

    let mut comando = Command::new(powershell);
    comando
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        // O caminho vai por variável de ambiente, nunca montado dentro do script.
        .env("MEMOTECA_ARQUIVO", arquivo)
        // Quem abre o app a partir do PowerShell 7 (ou um runner do GitHub) herda um
        // `PSModulePath` com módulos do PowerShell 7, que o Windows PowerShell 5.1 não
        // consegue carregar. Sem a variável, ele usa os caminhos padrão do sistema.
        .env_remove("PSModulePath")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CRIAR_SEM_JANELA: u32 = 0x0800_0000;
        comando.creation_flags(CRIAR_SEM_JANELA);
    }

    let mut filho = comando.spawn()?;
    let leitor = ler_em_thread(filho.stdout.take().expect("stdout foi pedido como pipe"));
    let leitor_de_erros = ler_em_thread(filho.stderr.take().expect("stderr foi pedido como pipe"));

    let limite = Instant::now() + PRAZO_DA_VERIFICACAO;
    loop {
        if filho.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= limite {
            let _ = filho.kill();
            let _ = filho.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "prazo esgotado"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let falha_na_leitura = || io::Error::other("a leitura da saída falhou");
    let saida = leitor.join().map_err(|_| falha_na_leitura())??;
    let erros = leitor_de_erros.join().map_err(|_| falha_na_leitura())??;
    Ok((saida, erros))
}

fn interpretar(saida: &str, publicador: &str) -> Result<(), FalhaDeAssinatura> {
    let saida: SaidaDoPowerShell =
        serde_json::from_str(saida.trim().trim_start_matches('\u{feff}'))
            .map_err(|erro| FalhaDeAssinatura::NaoFoiPossivelVerificar(erro.to_string()))?;

    match saida.status.as_str() {
        "Valid" => {}
        "NotSigned" => return Err(FalhaDeAssinatura::NaoAssinado),
        outro => return Err(FalhaDeAssinatura::Invalida(outro.to_owned())),
    }

    if publicador_confere(&saida.assunto, publicador) {
        Ok(())
    } else {
        Err(FalhaDeAssinatura::PublicadorDiferente(saida.assunto))
    }
}

/// O assunto do certificado (`CN=..., O=...`) tem `publicador` como organização (`O`) ou
/// como nome comum (`CN`)?
pub fn publicador_confere(assunto: &str, publicador: &str) -> bool {
    campos_do_assunto(assunto)
        .iter()
        .any(|(chave, valor)| (chave == "O" || chave == "CN") && valor == publicador)
}

/// Separa um assunto de certificado em pares `(campo, valor)`, respeitando aspas.
fn campos_do_assunto(assunto: &str) -> Vec<(String, String)> {
    let mut partes = Vec::new();
    let mut atual = String::new();
    let mut entre_aspas = false;
    for caractere in assunto.chars() {
        match caractere {
            '"' => {
                entre_aspas = !entre_aspas;
                atual.push(caractere);
            }
            ',' if !entre_aspas => partes.push(std::mem::take(&mut atual)),
            _ => atual.push(caractere),
        }
    }
    partes.push(atual);

    partes
        .iter()
        .filter_map(|parte| {
            let (chave, valor) = parte.trim().split_once('=')?;
            let valor = valor.trim();
            let valor = valor
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(valor);
            Some((chave.trim().to_owned(), valor.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Assunto real do certificado do Ollama (`Get-AuthenticodeSignature` no `ollama.exe`).
    const ASSUNTO_DO_OLLAMA: &str = "CN=Ollama Inc., O=Ollama Inc., L=Toronto, S=Ontario, C=CA, \
        SERIALNUMBER=2713355, OID.2.5.4.15=Private Organization, \
        OID.1.3.6.1.4.1.311.60.2.1.2=Ontario, OID.1.3.6.1.4.1.311.60.2.1.3=CA";

    #[test]
    fn reconhece_o_publicador_no_assunto_real_do_ollama() {
        assert!(publicador_confere(ASSUNTO_DO_OLLAMA, PUBLICADOR_DO_OLLAMA));
    }

    #[test]
    fn recusa_outros_publicadores_e_parecidos() {
        assert!(!publicador_confere(
            ASSUNTO_DO_OLLAMA,
            "Microsoft Corporation"
        ));
        assert!(!publicador_confere(
            "CN=Ollama Inc, O=Ollama Inc",
            PUBLICADOR_DO_OLLAMA
        ));
        assert!(!publicador_confere(
            "CN=Falso Ollama Inc., O=Falso",
            PUBLICADOR_DO_OLLAMA
        ));
        assert!(!publicador_confere("", PUBLICADOR_DO_OLLAMA));
    }

    #[test]
    fn valores_com_virgula_entre_aspas_nao_quebram_o_assunto() {
        let assunto = "CN=\"Ollama Inc., Filial\", O=\"Outra, Empresa\", C=CA";

        assert!(publicador_confere(assunto, "Ollama Inc., Filial"));
        assert!(!publicador_confere(assunto, "Ollama Inc."));
    }

    #[test]
    fn interpreta_a_saida_do_powershell() {
        let valida = format!(r#"{{"Status":"Valid","Subject":"{ASSUNTO_DO_OLLAMA}"}}"#);
        assert_eq!(interpretar(&valida, PUBLICADOR_DO_OLLAMA), Ok(()));

        assert_eq!(
            interpretar(
                r#"{"Status":"NotSigned","Subject":""}"#,
                PUBLICADOR_DO_OLLAMA
            ),
            Err(FalhaDeAssinatura::NaoAssinado)
        );
        assert_eq!(
            interpretar(
                r#"{"Status":"HashMismatch","Subject":"CN=Ollama Inc."}"#,
                PUBLICADOR_DO_OLLAMA
            ),
            Err(FalhaDeAssinatura::Invalida("HashMismatch".into()))
        );
        assert!(matches!(
            interpretar(
                r#"{"Status":"Valid","Subject":"CN=Outra Empresa"}"#,
                PUBLICADOR_DO_OLLAMA
            ),
            Err(FalhaDeAssinatura::PublicadorDiferente(_))
        ));
        assert!(matches!(
            interpretar("não é json", PUBLICADOR_DO_OLLAMA),
            Err(FalhaDeAssinatura::NaoFoiPossivelVerificar(_))
        ));
    }

    #[test]
    fn aceita_a_saida_com_marca_de_ordem_de_bytes() {
        let saida = format!("\u{feff}{{\"Status\":\"Valid\",\"Subject\":\"{ASSUNTO_DO_OLLAMA}\"}}");

        assert_eq!(interpretar(&saida, PUBLICADOR_DO_OLLAMA), Ok(()));
    }
}
