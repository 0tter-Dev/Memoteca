//! Log local com os detalhes técnicos dos erros (E14).
//!
//! O usuário nunca vê estes textos: a tela mostra só a mensagem amigável. Aqui ficam o erro
//! de verdade (código do SQLite, resposta do Ollama, caminho que falhou), para investigar um
//! problema depois.
//!
//! * Arquivo `logs/memoteca.log`, na pasta de dados do app, ao lado do banco.
//! * **Retenção:** cada arquivo vai até 1 MB e só os 3 mais recentes ficam
//!   (`memoteca.log`, `memoteca.1.log` e `memoteca.2.log`), uns 3 MB no máximo.
//! * **Privacidade:** só entram erros técnicos. O que o usuário escreve (mensagens, memórias,
//!   instruções) e o que o modelo responde nunca vai para o log.
//! * Se não for possível gravar o arquivo, o app segue sem log. Um log nunca derruba o app.
//!
//! Sem o log iniciado (nos testes, por exemplo), o texto vai para a saída de erro.

use std::fmt::Arguments;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tamanho máximo de um arquivo de log, em bytes.
pub const TAMANHO_MAXIMO: u64 = 1024 * 1024;
/// Quantos arquivos de log ficam guardados, contando o atual.
pub const ARQUIVOS_MANTIDOS: usize = 3;

/// Uma linha de log nunca passa deste tamanho, em caracteres.
const LINHA_MAXIMA: usize = 600;

const NOME: &str = "memoteca";

/// Escreve linhas num arquivo e faz o rodízio quando ele passa do limite.
pub struct Escritor {
    pasta: PathBuf,
    limite: u64,
    manter: usize,
    arquivo: Option<File>,
    tamanho: u64,
}

impl Escritor {
    pub fn novo(pasta: &Path, limite: u64, manter: usize) -> Self {
        let _ = fs::create_dir_all(pasta);
        let mut escritor = Self {
            pasta: pasta.to_path_buf(),
            limite,
            manter: manter.max(1),
            arquivo: None,
            tamanho: 0,
        };
        escritor.abrir();
        escritor
    }

    fn caminho(&self, indice: usize) -> PathBuf {
        if indice == 0 {
            self.pasta.join(format!("{NOME}.log"))
        } else {
            self.pasta.join(format!("{NOME}.{indice}.log"))
        }
    }

    fn abrir(&mut self) {
        let caminho = self.caminho(0);
        self.tamanho = fs::metadata(&caminho).map_or(0, |meta| meta.len());
        self.arquivo = OpenOptions::new()
            .create(true)
            .append(true)
            .open(caminho)
            .ok();
    }

    /// `memoteca.log` vira `memoteca.1.log`, que vira `memoteca.2.log`, e o mais antigo sai.
    fn girar(&mut self) {
        self.arquivo = None;
        let _ = fs::remove_file(self.caminho(self.manter - 1));
        for indice in (0..self.manter - 1).rev() {
            let origem = self.caminho(indice);
            if origem.exists() {
                let _ = fs::rename(origem, self.caminho(indice + 1));
            }
        }
        self.abrir();
    }

    /// Grava uma linha. Texto com quebras de linha vira uma linha só, para o arquivo continuar
    /// legível e uma mensagem não forjar outra linha.
    pub fn linha(&mut self, instante: &str, texto: &str) {
        let limpo: String = texto
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(LINHA_MAXIMA)
            .collect();
        let linha = format!("{instante} ERRO {limpo}\n");
        if self.tamanho + linha.len() as u64 > self.limite && self.tamanho > 0 {
            self.girar();
        }
        if let Some(arquivo) = self.arquivo.as_mut()
            && arquivo.write_all(linha.as_bytes()).is_ok()
        {
            self.tamanho += linha.len() as u64;
        }
    }
}

static LOG: OnceLock<Mutex<Escritor>> = OnceLock::new();

/// Liga o log na pasta dada (normalmente `<pasta de dados>/logs`). Só a primeira chamada vale.
pub fn iniciar(pasta: &Path) {
    let _ = LOG.set(Mutex::new(Escritor::novo(
        pasta,
        TAMANHO_MAXIMO,
        ARQUIVOS_MANTIDOS,
    )));
}

/// Pasta onde o log fica, dentro da pasta de dados do app.
pub fn pasta_do_log(pasta_de_dados: &Path) -> PathBuf {
    pasta_de_dados.join("logs")
}

/// Registra um erro técnico. Prefira o macro `registrar!`.
pub fn escrever(mensagem: Arguments<'_>) {
    let texto = mensagem.to_string();
    if let Some(escritor) = LOG.get()
        && let Ok(mut escritor) = escritor.lock()
    {
        escritor.linha(&agora(), &texto);
    } else {
        eprintln!("{texto}");
    }
}

/// Registra um erro técnico no log local, com o mesmo formato do `eprintln!`.
#[macro_export]
macro_rules! registrar {
    ($($argumento:tt)*) => {
        $crate::log::escrever(format_args!($($argumento)*))
    };
}

/// Agora, em UTC, no formato `2026-11-11T00:12:03.123Z`.
fn agora() -> String {
    let duracao = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    formatar_instante(duracao.as_secs(), duracao.subsec_millis())
}

/// Formata um instante (segundos desde 1970 e milissegundos) em UTC.
pub fn formatar_instante(segundos: u64, milissegundos: u32) -> String {
    let dias = (segundos / 86_400) as i64;
    let resto = segundos % 86_400;
    // Dias desde 1970-01-01 para data civil (algoritmo de Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let dia_da_era = z.rem_euclid(146_097);
    let ano_da_era =
        (dia_da_era - dia_da_era / 1_460 + dia_da_era / 36_524 - dia_da_era / 146_096) / 365;
    let dia_do_ano = dia_da_era - (365 * ano_da_era + ano_da_era / 4 - ano_da_era / 100);
    let mp = (5 * dia_do_ano + 2) / 153;
    let dia = dia_do_ano - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = ano_da_era + era * 400 + i64::from(mes <= 2);
    format!(
        "{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}.{milissegundos:03}Z",
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_nova(nome: &str) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!(
            "memoteca-log-{nome}-{}",
            crate::banco::id::novo_id()
        ));
        fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    fn ler(pasta: &Path, nome: &str) -> String {
        fs::read_to_string(pasta.join(nome)).unwrap_or_default()
    }

    #[test]
    fn formata_o_instante_em_utc() {
        assert_eq!(formatar_instante(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            formatar_instante(1_783_728_723, 123),
            "2026-07-11T00:12:03.123Z"
        );
        // Ano bissexto e virada de mês.
        assert_eq!(
            formatar_instante(951_782_400, 5),
            "2000-02-29T00:00:00.005Z"
        );
        assert_eq!(
            formatar_instante(1_709_251_199, 999),
            "2024-02-29T23:59:59.999Z"
        );
    }

    #[test]
    fn grava_uma_linha_por_erro_com_o_instante() {
        let pasta = pasta_nova("linha");
        let mut escritor = Escritor::novo(&pasta, 10_000, 3);

        escritor.linha(
            "2026-11-11T00:00:00.000Z",
            "banco indisponível: arquivo travado",
        );
        escritor.linha("2026-11-11T00:00:01.000Z", "outro erro");

        assert_eq!(
            ler(&pasta, "memoteca.log"),
            "2026-11-11T00:00:00.000Z ERRO banco indisponível: arquivo travado\n\
             2026-11-11T00:00:01.000Z ERRO outro erro\n"
        );
        let _ = fs::remove_dir_all(&pasta);
    }

    #[test]
    fn texto_com_quebras_de_linha_continua_sendo_uma_linha_so() {
        let pasta = pasta_nova("quebras");
        let mut escritor = Escritor::novo(&pasta, 10_000, 3);

        escritor.linha("t", "primeira\nsegunda\r\n2026-01-01 ERRO forjada");

        let conteudo = ler(&pasta, "memoteca.log");
        assert_eq!(conteudo.lines().count(), 1, "{conteudo:?}");
        let _ = fs::remove_dir_all(&pasta);
    }

    #[test]
    fn passou_do_limite_gira_os_arquivos_e_so_os_tres_mais_recentes_ficam() {
        let pasta = pasta_nova("rodizio");
        // Cada linha tem 14 bytes, e o limite de 40 faz caber 2 por arquivo.
        let mut escritor = Escritor::novo(&pasta, 40, 3);

        for numero in 0..10 {
            escritor.linha("t", &format!("erro {numero}"));
        }

        let mut arquivos: Vec<String> = fs::read_dir(&pasta)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        arquivos.sort();
        assert_eq!(
            arquivos,
            ["memoteca.1.log", "memoteca.2.log", "memoteca.log"]
        );
        // O mais recente fica no arquivo atual, e o mais antigo foi descartado.
        assert!(ler(&pasta, "memoteca.log").contains("erro 9"));
        let todos = format!(
            "{}{}{}",
            ler(&pasta, "memoteca.log"),
            ler(&pasta, "memoteca.1.log"),
            ler(&pasta, "memoteca.2.log")
        );
        assert!(!todos.contains("erro 0"), "o mais antigo deveria ter saído");
        for arquivo in ["memoteca.log", "memoteca.1.log", "memoteca.2.log"] {
            let tamanho = fs::metadata(pasta.join(arquivo)).unwrap().len();
            assert!(tamanho <= 40, "{arquivo} passou do limite: {tamanho}");
        }
        let _ = fs::remove_dir_all(&pasta);
    }

    #[test]
    fn reabrir_continua_no_mesmo_arquivo_sem_apagar_o_que_ja_estava() {
        let pasta = pasta_nova("reabrir");
        Escritor::novo(&pasta, 10_000, 3).linha("t", "antes de fechar o app");

        let mut depois = Escritor::novo(&pasta, 10_000, 3);
        depois.linha("t", "depois de abrir de novo");

        let conteudo = ler(&pasta, "memoteca.log");
        assert!(conteudo.contains("antes de fechar o app"));
        assert!(conteudo.contains("depois de abrir de novo"));
        let _ = fs::remove_dir_all(&pasta);
    }

    #[test]
    fn pasta_impossivel_de_criar_nao_derruba_nada() {
        // Um arquivo no lugar da pasta: criar a pasta e abrir o log falham.
        let pasta = pasta_nova("impossivel");
        let arquivo = pasta.join("ocupado");
        fs::write(&arquivo, "x").unwrap();

        let mut escritor = Escritor::novo(&arquivo.join("logs"), 100, 3);
        escritor.linha("t", "não vai a lugar nenhum, e não pode dar pânico");

        let _ = fs::remove_dir_all(&pasta);
    }
}
