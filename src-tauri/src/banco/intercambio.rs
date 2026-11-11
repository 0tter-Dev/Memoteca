//! Exportar e importar os dados (E13): o backup manual.
//!
//! O backup é **um único arquivo JSON** com a versão do formato, e traz projetos, conversas,
//! mensagens (inclusive o raciocínio) e memórias, com os mesmos IDs e datas. As configurações
//! do app não entram: elas são da máquina (por exemplo, se o onboarding foi concluído).
//!
//! **Importar mescla**, sem nunca apagar nem sobrescrever nada:
//!
//! * um item cujo ID já existe aqui é **mantido como está**, sem duplicar. Uma conversa que já
//!   existe é mantida inteira, com as mensagens que tem, e as mensagens do arquivo não entram;
//! * um item que o usuário já excluiu neste computador é **ignorado**, porque o banco não
//!   aceita de volta um ID excluído. O que depende dele (as conversas de um projeto excluído,
//!   por exemplo) também é ignorado;
//! * o resto é adicionado. Num banco vazio, isso restaura tudo.
//!
//! A importação é **atômica**: roda numa transação e, se qualquer coisa falhar, nada fica
//! importado pela metade. Um arquivo que não passa na verificação é recusado antes de qualquer
//! gravação.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::Banco;
use super::erro::ErroBanco;
use super::modelos::{
    Conversa, DadosConversa, DadosProjeto, EscopoMemoria, Memoria, Mensagem, Projeto,
};
use super::repositorio::{
    conversa_da_linha, memoria_da_linha, mensagem_da_linha, projeto_da_linha, validar_conversa,
    validar_memoria, validar_projeto,
};

/// Marca do arquivo: sem ela, o arquivo não é um backup do Memoteca.
pub const FORMATO: &str = "memoteca-backup";
/// Versão do formato que este app escreve e sabe ler.
pub const VERSAO_DO_FORMATO: u32 = 1;

/// O conteúdo do arquivo de backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub formato: String,
    pub versao: u32,
    /// Quando o backup foi feito (UTC, ISO 8601).
    pub exportado_em: String,
    /// Versão do Memoteca que fez o backup.
    pub app: String,
    pub projetos: Vec<Projeto>,
    pub conversas: Vec<Conversa>,
    pub mensagens: Vec<Mensagem>,
    pub memorias: Vec<Memoria>,
}

/// Quantidade de itens de um tipo, por destino na importação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contagem {
    /// Seriam (ou foram) adicionados.
    pub novos: u64,
    /// O ID já existe aqui, então o que está aqui fica como está.
    pub ja_existem: u64,
    /// Foram excluídos neste computador (ou dependem de algo excluído) e não voltam.
    pub ignorados: u64,
}

/// O que a importação faz (ou fez), tipo por tipo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoDaImportacao {
    pub projetos: Contagem,
    pub conversas: Contagem,
    pub mensagens: Contagem,
    pub memorias: Contagem,
}

impl ResumoDaImportacao {
    pub fn total_de_novos(&self) -> u64 {
        self.projetos.novos + self.conversas.novos + self.mensagens.novos + self.memorias.novos
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErroDeBackup {
    /// Não foi possível abrir ou ler o arquivo.
    ArquivoIlegivel(String),
    /// O arquivo é grande demais para ser um backup.
    ArquivoGrandeDemais,
    /// O arquivo não é um backup do Memoteca, ou está truncado ou danificado.
    NaoEBackup(String),
    /// O backup é de um formato que este app não conhece.
    VersaoDesconhecida(Option<u64>),
    /// O arquivo tem a estrutura certa, mas dados que não passam na verificação.
    Invalido(String),
    Banco(ErroBanco),
}

impl ErroDeBackup {
    /// Mensagem em pt-BR para o usuário. O detalhe técnico fica só no log.
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::ArquivoIlegivel(_) => "Não foi possível ler o arquivo. Confira se ele ainda \
                existe e se o Memoteca tem permissão para abri-lo. Nada foi alterado."
                .to_owned(),
            Self::ArquivoGrandeDemais => "Este arquivo é grande demais para ser um backup do \
                Memoteca. Nada foi alterado. Escolha outro arquivo."
                .to_owned(),
            Self::NaoEBackup(_) => "Este arquivo não é um backup do Memoteca, ou está \
                incompleto ou danificado. Nada foi importado nem alterado. Escolha outro \
                arquivo."
                .to_owned(),
            Self::VersaoDesconhecida(Some(versao)) => format!(
                "Este backup usa o formato {versao}, que esta versão do Memoteca não conhece. \
                 Atualize o Memoteca e tente de novo. Nada foi importado nem alterado."
            ),
            Self::VersaoDesconhecida(None) => "Não foi possível saber a versão deste backup, \
                então ele não foi importado. Nada foi alterado."
                .to_owned(),
            Self::Invalido(_) => "Este backup tem dados que não passam na verificação, então \
                não foi importado. Nada foi alterado. Escolha outro arquivo."
                .to_owned(),
            Self::Banco(erro) => erro.mensagem_amigavel(),
        }
    }
}

impl std::fmt::Display for ErroDeBackup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArquivoIlegivel(detalhe) => write!(f, "arquivo ilegível: {detalhe}"),
            Self::ArquivoGrandeDemais => write!(f, "arquivo grande demais"),
            Self::NaoEBackup(detalhe) => write!(f, "o arquivo não é um backup: {detalhe}"),
            Self::VersaoDesconhecida(Some(versao)) => {
                write!(f, "formato de backup {versao} desconhecido")
            }
            Self::VersaoDesconhecida(None) => write!(f, "o backup não informa a versão"),
            Self::Invalido(detalhe) => write!(f, "backup inválido: {detalhe}"),
            Self::Banco(erro) => write!(f, "{erro}"),
        }
    }
}

impl std::error::Error for ErroDeBackup {}

impl From<ErroBanco> for ErroDeBackup {
    fn from(erro: ErroBanco) -> Self {
        Self::Banco(erro)
    }
}

impl From<rusqlite::Error> for ErroDeBackup {
    fn from(erro: rusqlite::Error) -> Self {
        Self::Banco(erro.into())
    }
}

fn invalido(detalhe: impl Into<String>) -> ErroDeBackup {
    ErroDeBackup::Invalido(detalhe.into())
}

/// Uma data no formato que o app grava: `2026-11-10T03:14:40.123Z` (de 1 a 9 dígitos de fração).
fn data_valida(texto: &str) -> bool {
    let bytes = texto.as_bytes();
    if bytes.len() < 20 || !texto.is_ascii() {
        return false;
    }
    let digitos = |de: usize, ate: usize| bytes[de..ate].iter().all(u8::is_ascii_digit);
    let modelo = digitos(0, 4)
        && bytes[4] == b'-'
        && digitos(5, 7)
        && bytes[7] == b'-'
        && digitos(8, 10)
        && bytes[10] == b'T'
        && digitos(11, 13)
        && bytes[13] == b':'
        && digitos(14, 16)
        && bytes[16] == b':'
        && digitos(17, 19);
    if !modelo || bytes[bytes.len() - 1] != b'Z' {
        return false;
    }
    let resto = &bytes[19..bytes.len() - 1];
    match resto {
        [] => true,
        [b'.', fracao @ ..] => {
            !fracao.is_empty() && fracao.len() <= 9 && fracao.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

impl Backup {
    /// Lê e **verifica** o conteúdo de um arquivo de backup. Nada é gravado aqui.
    ///
    /// A ordem importa: primeiro vê se é JSON e se tem a marca e a versão do formato (assim
    /// um formato novo é reconhecido como "versão desconhecida", e não como arquivo inválido),
    /// e só depois lê a estrutura e confere os dados.
    pub fn ler(bytes: &[u8]) -> Result<Self, ErroDeBackup> {
        let valor: Value = serde_json::from_slice(bytes)
            .map_err(|erro| ErroDeBackup::NaoEBackup(erro.to_string()))?;
        let Some(objeto) = valor.as_object() else {
            return Err(ErroDeBackup::NaoEBackup("o JSON não é um objeto".into()));
        };
        if objeto.get("formato").and_then(Value::as_str) != Some(FORMATO) {
            return Err(ErroDeBackup::NaoEBackup("sem a marca do formato".into()));
        }
        match objeto.get("versao").and_then(Value::as_u64) {
            Some(versao) if versao == u64::from(VERSAO_DO_FORMATO) => {}
            outra => return Err(ErroDeBackup::VersaoDesconhecida(outra)),
        }
        let backup: Self = serde_json::from_value(valor)
            .map_err(|erro| ErroDeBackup::NaoEBackup(erro.to_string()))?;
        backup.verificar()?;
        Ok(backup)
    }

    /// Confere os dados: IDs válidos e únicos, referências que fecham dentro do arquivo, datas
    /// e campos nas mesmas regras de quando o usuário os cria.
    pub fn verificar(&self) -> Result<(), ErroDeBackup> {
        fn ids_unicos<'a>(
            tipo: &str,
            ids: impl Iterator<Item = &'a str>,
        ) -> Result<HashSet<&'a str>, ErroDeBackup> {
            let mut vistos = HashSet::new();
            for id in ids {
                if Uuid::parse_str(id).is_err() {
                    return Err(invalido(format!("{tipo} com ID que não é válido")));
                }
                if !vistos.insert(id) {
                    return Err(invalido(format!("{tipo} com ID repetido")));
                }
            }
            Ok(vistos)
        }
        let datas = |criado: &str, atualizado: &str| -> Result<(), ErroDeBackup> {
            if data_valida(criado) && data_valida(atualizado) {
                Ok(())
            } else {
                Err(invalido("data fora do formato"))
            }
        };
        fn regra<T>(resultado: Result<T, ErroBanco>) -> Result<(), ErroDeBackup> {
            resultado
                .map(|_| ())
                .map_err(|erro| invalido(erro.to_string()))
        }

        if !data_valida(&self.exportado_em) {
            return Err(invalido("data do backup fora do formato"));
        }

        let projetos = ids_unicos("projeto", self.projetos.iter().map(|p| p.id.as_str()))?;
        let conversas = ids_unicos("conversa", self.conversas.iter().map(|c| c.id.as_str()))?;
        ids_unicos("mensagem", self.mensagens.iter().map(|m| m.id.as_str()))?;
        ids_unicos("memória", self.memorias.iter().map(|m| m.id.as_str()))?;

        for projeto in &self.projetos {
            datas(&projeto.criado_em, &projeto.atualizado_em)?;
            regra(validar_projeto(&DadosProjeto {
                nome: projeto.nome.clone(),
                system_prompt: projeto.system_prompt.clone(),
                modelo_padrao: projeto.modelo_padrao.clone(),
            }))?;
        }
        for conversa in &self.conversas {
            datas(&conversa.criado_em, &conversa.atualizado_em)?;
            if !projetos.contains(conversa.projeto_id.as_str()) {
                return Err(invalido("conversa de um projeto que não está no arquivo"));
            }
            regra(validar_conversa(&DadosConversa {
                titulo: conversa.titulo.clone(),
                modelo: conversa.modelo.clone(),
                temperature: conversa.temperature,
                num_ctx: conversa.num_ctx,
                keep_alive: conversa.keep_alive.clone(),
            }))?;
        }
        let mut posicoes: HashSet<(&str, i64)> = HashSet::new();
        for mensagem in &self.mensagens {
            datas(&mensagem.criado_em, &mensagem.atualizado_em)?;
            if !conversas.contains(mensagem.conversa_id.as_str()) {
                return Err(invalido("mensagem de uma conversa que não está no arquivo"));
            }
            if mensagem.posicao < 1 {
                return Err(invalido("mensagem com posição inválida"));
            }
            if !posicoes.insert((mensagem.conversa_id.as_str(), mensagem.posicao)) {
                return Err(invalido("duas mensagens na mesma posição da conversa"));
            }
        }
        for memoria in &self.memorias {
            datas(&memoria.criado_em, &memoria.atualizado_em)?;
            regra(validar_memoria(&memoria.conteudo))?;
            match (memoria.escopo, memoria.projeto_id.as_deref()) {
                (EscopoMemoria::Global, None) => {}
                (EscopoMemoria::Projeto, Some(projeto)) if projetos.contains(projeto) => {}
                _ => return Err(invalido("memória com escopo e projeto que não combinam")),
            }
        }
        Ok(())
    }
}

/// IDs que já existem em cada tabela, e os que foram excluídos.
struct Presentes {
    projetos: HashSet<String>,
    conversas: HashSet<String>,
    mensagens: HashSet<String>,
    memorias: HashSet<String>,
    excluidos: HashSet<String>,
}

fn ids_de(conexao: &Connection, tabela: &str) -> rusqlite::Result<HashSet<String>> {
    let mut consulta = conexao.prepare(&format!("SELECT id FROM {tabela}"))?;
    consulta
        .query_map([], |linha| linha.get::<_, String>(0))?
        .collect()
}

impl Presentes {
    fn ler(conexao: &Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            projetos: ids_de(conexao, "projetos")?,
            conversas: ids_de(conexao, "conversas")?,
            mensagens: ids_de(conexao, "mensagens")?,
            memorias: ids_de(conexao, "memorias")?,
            excluidos: ids_de(conexao, "exclusoes")?,
        })
    }
}

/// O que acontece com uma conversa do arquivo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SituacaoDaConversa {
    /// Entra agora, com as mensagens dela.
    Nova,
    /// Já existe aqui: fica como está, com as mensagens que tem.
    Existente,
    /// Foi excluída aqui, ou depende de algo excluído.
    Ignorada,
}

impl Banco {
    /// Monta o backup com todos os dados, numa leitura só (uma transação), para que as tabelas
    /// sejam do mesmo instante.
    pub fn montar_backup(&self) -> Result<Backup, ErroBanco> {
        let leitura = self.conexao.unchecked_transaction()?;
        let projetos = self.ler_tudo("projetos", "criado_em, id", projeto_da_linha)?;
        let conversas = self.ler_tudo("conversas", "criado_em, id", conversa_da_linha)?;
        let mensagens = self.ler_tudo("mensagens", "conversa_id, posicao", mensagem_da_linha)?;
        let memorias = self.ler_tudo("memorias", "criado_em, id", memoria_da_linha)?;
        let exportado_em: String = self.conexao.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            [],
            |linha| linha.get(0),
        )?;
        leitura.commit()?;

        Ok(Backup {
            formato: FORMATO.to_owned(),
            versao: VERSAO_DO_FORMATO,
            exportado_em,
            app: env!("CARGO_PKG_VERSION").to_owned(),
            projetos,
            conversas,
            mensagens,
            memorias,
        })
    }

    fn ler_tudo<T>(
        &self,
        tabela: &str,
        ordem: &str,
        linha: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, ErroBanco> {
        let mut consulta = self
            .conexao
            .prepare(&format!("SELECT * FROM {tabela} ORDER BY {ordem}"))?;
        let itens = consulta
            .query_map([], linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(itens)
    }

    /// O que a importação faria com este backup, **sem gravar nada**. É a base da tela de
    /// confirmação.
    pub fn analisar_backup(&self, backup: &Backup) -> Result<ResumoDaImportacao, ErroDeBackup> {
        self.aplicar_backup(backup, false)
    }

    /// Importa o backup, mesclando como descrito no topo do módulo, **de forma atômica**.
    /// Devolve o que foi feito de fato.
    pub fn importar_backup(&self, backup: &Backup) -> Result<ResumoDaImportacao, ErroDeBackup> {
        // O backup pode ter chegado sem passar por `Backup::ler`. Confere de novo.
        backup.verificar()?;
        self.aplicar_backup(backup, true)
    }

    /// Faz tudo dentro de uma transação. Sem `gravar`, ou se algo falhar no meio, a transação
    /// é desfeita (ao sair sem `commit`), e o banco fica exatamente como estava.
    fn aplicar_backup(
        &self,
        backup: &Backup,
        gravar: bool,
    ) -> Result<ResumoDaImportacao, ErroDeBackup> {
        let transacao = self.conexao.unchecked_transaction()?;
        let presentes = Presentes::ler(&self.conexao)?;
        let mut resumo = ResumoDaImportacao::default();

        // Projetos. `utilizavel`: o ID existe aqui ou acabou de entrar, então dá para pendurar
        // conversas e memórias nele.
        let mut utilizavel: HashMap<&str, bool> = HashMap::new();
        {
            let mut inserir = self.conexao.prepare_cached(
                "INSERT INTO projetos (id, nome, system_prompt, modelo_padrao, criado_em, atualizado_em)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for projeto in &backup.projetos {
                if presentes.projetos.contains(&projeto.id) {
                    resumo.projetos.ja_existem += 1;
                    utilizavel.insert(&projeto.id, true);
                } else if presentes.excluidos.contains(&projeto.id) {
                    resumo.projetos.ignorados += 1;
                    utilizavel.insert(&projeto.id, false);
                } else {
                    inserir.execute(params![
                        projeto.id,
                        projeto.nome,
                        projeto.system_prompt,
                        projeto.modelo_padrao,
                        projeto.criado_em,
                        projeto.atualizado_em
                    ])?;
                    resumo.projetos.novos += 1;
                    utilizavel.insert(&projeto.id, true);
                }
            }
        }

        // Conversas.
        let mut situacao: HashMap<&str, SituacaoDaConversa> = HashMap::new();
        {
            let mut inserir = self.conexao.prepare_cached(
                "INSERT INTO conversas (id, projeto_id, titulo, modelo, temperature, num_ctx, keep_alive, criado_em, atualizado_em)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for conversa in &backup.conversas {
                let projeto_serve = utilizavel
                    .get(conversa.projeto_id.as_str())
                    .copied()
                    .unwrap_or(false);
                let destino = if !projeto_serve || presentes.excluidos.contains(&conversa.id) {
                    resumo.conversas.ignorados += 1;
                    SituacaoDaConversa::Ignorada
                } else if presentes.conversas.contains(&conversa.id) {
                    resumo.conversas.ja_existem += 1;
                    SituacaoDaConversa::Existente
                } else {
                    inserir.execute(params![
                        conversa.id,
                        conversa.projeto_id,
                        conversa.titulo,
                        conversa.modelo,
                        conversa.temperature,
                        conversa.num_ctx,
                        conversa.keep_alive,
                        conversa.criado_em,
                        conversa.atualizado_em
                    ])?;
                    resumo.conversas.novos += 1;
                    SituacaoDaConversa::Nova
                };
                situacao.insert(&conversa.id, destino);
            }
        }

        // Mensagens: só entram as de uma conversa nova. Uma conversa que já existe fica como
        // está, e uma ignorada leva as mensagens junto.
        {
            let mut inserir = self.conexao.prepare_cached(
                "INSERT INTO mensagens (id, conversa_id, posicao, papel, conteudo, raciocinio, estado, criado_em, atualizado_em)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for mensagem in &backup.mensagens {
                match situacao.get(mensagem.conversa_id.as_str()) {
                    Some(SituacaoDaConversa::Nova)
                        if !presentes.mensagens.contains(&mensagem.id)
                            && !presentes.excluidos.contains(&mensagem.id) =>
                    {
                        inserir.execute(params![
                            mensagem.id,
                            mensagem.conversa_id,
                            mensagem.posicao,
                            mensagem.papel.como_texto(),
                            mensagem.conteudo,
                            mensagem.raciocinio,
                            mensagem.estado.como_texto(),
                            mensagem.criado_em,
                            mensagem.atualizado_em
                        ])?;
                        resumo.mensagens.novos += 1;
                    }
                    Some(SituacaoDaConversa::Existente) => resumo.mensagens.ja_existem += 1,
                    _ => resumo.mensagens.ignorados += 1,
                }
            }
        }

        // Memórias.
        {
            let mut inserir = self.conexao.prepare_cached(
                "INSERT INTO memorias (id, escopo, projeto_id, conteudo, criado_em, atualizado_em)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for memoria in &backup.memorias {
                let projeto_serve = match memoria.projeto_id.as_deref() {
                    None => true,
                    Some(projeto) => utilizavel.get(projeto).copied().unwrap_or(false),
                };
                if !projeto_serve || presentes.excluidos.contains(&memoria.id) {
                    resumo.memorias.ignorados += 1;
                } else if presentes.memorias.contains(&memoria.id) {
                    resumo.memorias.ja_existem += 1;
                } else {
                    inserir.execute(params![
                        memoria.id,
                        match memoria.escopo {
                            EscopoMemoria::Global => "global",
                            EscopoMemoria::Projeto => "projeto",
                        },
                        memoria.projeto_id,
                        memoria.conteudo,
                        memoria.criado_em,
                        memoria.atualizado_em
                    ])?;
                    resumo.memorias.novos += 1;
                }
            }
        }

        if gravar {
            transacao.commit()?;
        }
        // Sem `commit`, `transacao` desfaz tudo ao sair daqui.
        Ok(resumo)
    }

    /// Data de hoje (hora local) no formato `AAAA-MM-DD`, para sugerir o nome do arquivo.
    pub fn data_local_de_hoje(&self) -> Result<String, ErroBanco> {
        Ok(self.conexao.query_row(
            "SELECT strftime('%Y-%m-%d', 'now', 'localtime')",
            [],
            |linha| linha.get(0),
        )?)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Papel;

    type Mutacao = Box<dyn Fn(&mut Backup)>;

    fn banco_com_dados() -> (Banco, Backup) {
        let banco = Banco::em_memoria().unwrap();
        let projeto = banco
            .criar_projeto(&DadosProjeto {
                nome: "Estudos".into(),
                system_prompt: "Seja breve.".into(),
                modelo_padrao: Some("qwen3:8b".into()),
            })
            .unwrap();
        let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
        banco
            .adicionar_mensagem(&conversa.id, Papel::Usuario, "Oi", None)
            .unwrap();
        banco
            .adicionar_mensagem(&conversa.id, Papel::Assistente, "Olá!", Some("pensei"))
            .unwrap();
        banco.criar_memoria_global("Mora em Recife").unwrap();
        banco
            .criar_memoria_de_projeto(&projeto.id, "Usa Rust")
            .unwrap();
        let backup = banco.montar_backup().unwrap();
        (banco, backup)
    }

    fn contar(banco: &Banco, tabela: &str) -> i64 {
        banco
            .conexao
            .query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |l| l.get(0))
            .unwrap()
    }

    #[test]
    fn datas_do_app_passam_e_formatos_errados_nao() {
        for boa in [
            "2026-11-10T03:14:40.123Z",
            "2026-11-10T03:14:40Z",
            "2026-11-10T03:14:40.123456789Z",
        ] {
            assert!(data_valida(boa), "{boa}");
        }
        for ruim in [
            "",
            "ontem",
            "2026-11-10 03:14:40.123Z",
            "2026-11-10T03:14:40.123",
            "2026-11-10T03:14:40.Z",
            "2026-11-10T03:14:40.1234567890Z",
            "2026-1-10T03:14:40.123Z",
            "2026-11-10T03:14:40+03:00",
            "２０２６-11-10T03:14:40.123Z",
        ] {
            assert!(!data_valida(ruim), "{ruim}");
        }
    }

    #[test]
    fn o_backup_exportado_passa_na_propria_verificacao_e_volta_igual_pelo_json() {
        let (_, backup) = banco_com_dados();

        let bytes = serde_json::to_vec_pretty(&backup).unwrap();
        let lido = Backup::ler(&bytes).unwrap();

        assert_eq!(lido, backup);
        assert_eq!(lido.formato, FORMATO);
        assert_eq!(lido.versao, VERSAO_DO_FORMATO);
    }

    /// R29: uma falha no meio da gravação desfaz tudo. A falha é injetada com um gatilho que
    /// recusa uma mensagem específica, depois de projetos, conversas e outras mensagens já
    /// terem sido inseridos na mesma transação.
    #[test]
    fn r29_falha_no_meio_da_gravacao_nao_deixa_nada_importado_pela_metade() {
        let (_, backup) = banco_com_dados();
        let destino = Banco::em_memoria().unwrap();
        destino
            .conexao
            .execute_batch(
                "CREATE TRIGGER falha_injetada BEFORE INSERT ON mensagens
                 WHEN NEW.conteudo = 'Olá!'
                 BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
            )
            .unwrap();

        let resultado = destino.importar_backup(&backup);

        assert!(resultado.is_err(), "a importação deveria falhar");
        for tabela in ["projetos", "conversas", "mensagens", "memorias"] {
            assert_eq!(contar(&destino, tabela), 0, "{tabela} ficou pela metade");
        }
        // E o banco continua utilizável: sem o gatilho, a mesma importação funciona.
        destino
            .conexao
            .execute_batch("DROP TRIGGER falha_injetada;")
            .unwrap();
        destino.importar_backup(&backup).unwrap();
        assert_eq!(contar(&destino, "mensagens"), 2);
    }

    #[test]
    fn analisar_nao_grava_nada_e_conta_o_mesmo_que_a_importacao_faz() {
        let (_, backup) = banco_com_dados();
        let destino = Banco::em_memoria().unwrap();

        let previsto = destino.analisar_backup(&backup).unwrap();

        for tabela in ["projetos", "conversas", "mensagens", "memorias"] {
            assert_eq!(contar(&destino, tabela), 0, "{tabela}");
        }
        let feito = destino.importar_backup(&backup).unwrap();
        assert_eq!(previsto, feito);
        assert_eq!(feito.projetos.novos, 1);
        assert_eq!(feito.conversas.novos, 1);
        assert_eq!(feito.mensagens.novos, 2);
        assert_eq!(feito.memorias.novos, 2);
    }

    #[test]
    fn a_verificacao_recusa_ids_repetidos_referencias_soltas_e_dados_invalidos() {
        let (_, base) = banco_com_dados();
        let mutacoes: Vec<(&str, Mutacao)> = vec![
            (
                "id que não é UUID",
                Box::new(|b| b.projetos[0].id = "abc".into()),
            ),
            (
                "id de projeto repetido",
                Box::new(|b| {
                    let copia = b.projetos[0].clone();
                    b.projetos.push(copia);
                }),
            ),
            (
                "conversa sem projeto",
                Box::new(|b| b.conversas[0].projeto_id = Uuid::now_v7().to_string()),
            ),
            (
                "mensagem sem conversa",
                Box::new(|b| b.mensagens[0].conversa_id = Uuid::now_v7().to_string()),
            ),
            (
                "mesma posição",
                Box::new(|b| b.mensagens[1].posicao = b.mensagens[0].posicao),
            ),
            ("posição zero", Box::new(|b| b.mensagens[0].posicao = 0)),
            (
                "memória de projeto sem projeto",
                Box::new(|b| {
                    let m = b
                        .memorias
                        .iter_mut()
                        .find(|m| m.projeto_id.is_some())
                        .unwrap();
                    m.projeto_id = None;
                }),
            ),
            (
                "memória global com projeto",
                Box::new(|b| {
                    let projeto = b.projetos[0].id.clone();
                    let m = b
                        .memorias
                        .iter_mut()
                        .find(|m| m.projeto_id.is_none())
                        .unwrap();
                    m.projeto_id = Some(projeto);
                }),
            ),
            (
                "nome de projeto vazio",
                Box::new(|b| b.projetos[0].nome = "  ".into()),
            ),
            (
                "temperatura fora do limite",
                Box::new(|b| b.conversas[0].temperature = Some(9.0)),
            ),
            (
                "memória grande demais",
                Box::new(|b| b.memorias[0].conteudo = "a".repeat(5_000)),
            ),
            (
                "data inválida",
                Box::new(|b| b.mensagens[0].criado_em = "ontem".into()),
            ),
            (
                "data do backup inválida",
                Box::new(|b| b.exportado_em = String::new()),
            ),
        ];

        for (nome, mutar) in mutacoes {
            let mut backup = base.clone();
            mutar(&mut backup);
            assert!(
                matches!(backup.verificar(), Err(ErroDeBackup::Invalido(_))),
                "deveria recusar: {nome}"
            );
            // E a importação também recusa, sem tocar em nada.
            let destino = Banco::em_memoria().unwrap();
            assert!(destino.importar_backup(&backup).is_err(), "{nome}");
            assert_eq!(contar(&destino, "projetos"), 0, "{nome}");
        }
    }

    #[test]
    fn a_ordem_das_checagens_distingue_nao_backup_de_versao_desconhecida() {
        let (_, backup) = banco_com_dados();
        let mut valor = serde_json::to_value(&backup).unwrap();

        // Versão nova: é reconhecida como versão desconhecida, mesmo que o resto mude.
        valor["versao"] = serde_json::json!(2);
        valor["projetos"] = serde_json::json!("formato novo, outra estrutura");
        let erro = Backup::ler(valor.to_string().as_bytes()).unwrap_err();
        assert_eq!(erro, ErroDeBackup::VersaoDesconhecida(Some(2)));
        assert!(erro.mensagem_amigavel().contains("formato 2"));

        // Sem versão, ou com a versão em texto.
        valor["versao"] = serde_json::json!("1");
        assert_eq!(
            Backup::ler(valor.to_string().as_bytes()).unwrap_err(),
            ErroDeBackup::VersaoDesconhecida(None)
        );

        // Sem a marca do formato, ou nem objeto.
        for texto in [
            r#"{"versao":1}"#,
            "[]",
            "42",
            r#"{"formato":"outro","versao":1}"#,
        ] {
            assert!(
                matches!(
                    Backup::ler(texto.as_bytes()),
                    Err(ErroDeBackup::NaoEBackup(_))
                ),
                "{texto}"
            );
        }
    }

    #[test]
    fn mensagens_amigaveis_nao_vazam_detalhe_tecnico() {
        for erro in [
            ErroDeBackup::NaoEBackup("expected value at line 1 column 1".into()),
            ErroDeBackup::Invalido("memória grande demais SQLITE".into()),
            ErroDeBackup::VersaoDesconhecida(None),
        ] {
            let texto = erro.mensagem_amigavel();
            assert!(texto.contains("Nada foi"), "{texto}");
            assert!(!texto.contains("expected value"), "{texto}");
            assert!(!texto.contains("SQLITE"), "{texto}");
        }
    }
}
