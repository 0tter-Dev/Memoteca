//! Testes de aceite da E04: persistência, migrações, regras de memória e IDs.
//! Os testes dos cenários do plano de robustez citam o identificador `Rnn` no nome.

use std::fs;
use std::path::{Path, PathBuf};

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::banco::migracoes::{self, MIGRACOES, Migracao};
use memoteca_lib::banco::{Banco, DadosConversa, DadosProjeto, ErroBanco, EscopoMemoria, Papel};
use rusqlite::Connection;

/// Pasta temporária única, apagada ao fim do teste.
struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    fn nova() -> Self {
        let caminho = std::env::temp_dir().join(format!("memoteca-teste-{}", novo_id()));
        fs::create_dir_all(&caminho).expect("criar a pasta temporária");
        Self(caminho)
    }

    fn arquivo_do_banco(&self) -> PathBuf {
        self.0.join("dados").join("memoteca.db")
    }

    fn pasta_de_copias(&self) -> PathBuf {
        self.0.join("dados").join("backups")
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Conexão direta ao arquivo, para consultas e SQL cru que a API do app não permite.
fn conexao_crua(arquivo: &Path) -> Connection {
    let conexao = Connection::open(arquivo).expect("abrir o banco");
    conexao
        .pragma_update(None, "foreign_keys", true)
        .expect("ligar chaves estrangeiras");
    conexao
}

fn contar(conexao: &Connection, sql: &str) -> i64 {
    conexao
        .query_row(sql, [], |linha| linha.get(0))
        .expect("contar linhas")
}

fn esquema(conexao: &Connection) -> Vec<(String, String, Option<String>)> {
    let mut consulta = conexao
        .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap();
    consulta
        .query_map([], |linha| {
            Ok((linha.get(0)?, linha.get(1)?, linha.get(2)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn copias_na_pasta(pasta: &Path) -> Vec<PathBuf> {
    match fs::read_dir(pasta) {
        Ok(entradas) => entradas.filter_map(Result::ok).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn projeto(nome: &str) -> DadosProjeto {
    DadosProjeto {
        nome: nome.to_owned(),
        system_prompt: "Responda em português.".to_owned(),
        modelo_padrao: Some("llama3.2:latest".to_owned()),
    }
}

fn conversa(titulo: &str) -> DadosConversa {
    DadosConversa {
        titulo: titulo.to_owned(),
        modelo: None,
        temperature: None,
        num_ctx: None,
        keep_alive: None,
    }
}

const V1: Migracao = Migracao {
    versao: 1,
    nome: "tabela de itens",
    sql: "CREATE TABLE itens (id INTEGER PRIMARY KEY, nome TEXT NOT NULL);",
};

const V2: Migracao = Migracao {
    versao: 2,
    nome: "coluna de nota",
    sql: "ALTER TABLE itens ADD COLUMN nota TEXT;
          CREATE INDEX itens_por_nome ON itens (nome);",
};

/// Falha na segunda instrução, depois de uma primeira que daria certo.
const V3_QUEBRADA: Migracao = Migracao {
    versao: 3,
    nome: "migração quebrada",
    sql: "CREATE TABLE sobras (id INTEGER PRIMARY KEY);
          INSERT INTO tabela_que_nao_existe VALUES (1);",
};

// ----- Persistência -----

#[test]
fn dados_gravados_sao_encontrados_depois_de_fechar_e_reabrir() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();

    let (id_projeto, id_conversa, id_global) = {
        let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
        let p = banco.criar_projeto(&projeto("Estudos")).unwrap();
        let c = banco
            .criar_conversa(&p.id, &conversa("Primeira conversa"))
            .unwrap();
        banco
            .adicionar_mensagem(&c.id, Papel::Usuario, "Olá", None)
            .unwrap();
        banco
            .adicionar_mensagem(&c.id, Papel::Assistente, "Oi!", Some("pensando"))
            .unwrap();
        banco
            .criar_memoria_de_projeto(&p.id, "Gosta de Rust")
            .unwrap();
        let g = banco.criar_memoria_global("Usa Windows 11").unwrap();
        banco
            .definir_configuracao("onboarding", "concluido")
            .unwrap();
        (p.id, c.id, g.id)
    };

    let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
    let p = banco.obter_projeto(&id_projeto).unwrap();
    assert_eq!(p.nome, "Estudos");
    assert_eq!(p.system_prompt, "Responda em português.");
    assert_eq!(p.modelo_padrao.as_deref(), Some("llama3.2:latest"));

    let conversas = banco.listar_conversas_do_projeto(&id_projeto).unwrap();
    assert_eq!(conversas.len(), 1);
    assert_eq!(conversas[0].id, id_conversa);

    let mensagens = banco.listar_mensagens(&id_conversa).unwrap();
    assert_eq!(mensagens.len(), 2);
    assert_eq!(mensagens[0].conteudo, "Olá");
    assert_eq!(mensagens[1].raciocinio.as_deref(), Some("pensando"));

    let de_projeto = banco.listar_memorias_do_projeto(&id_projeto).unwrap();
    assert_eq!(de_projeto.len(), 1);
    assert_eq!(
        banco.obter_memoria(&id_global).unwrap().conteudo,
        "Usa Windows 11"
    );
    assert_eq!(
        banco.obter_configuracao("onboarding").unwrap().as_deref(),
        Some("concluido")
    );
}

// ----- Migrações -----

#[test]
fn migracoes_aplicam_em_banco_vazio() {
    let pasta = PastaTemporaria::nova();
    let banco = Banco::abrir(&pasta.arquivo_do_banco(), &pasta.pasta_de_copias()).unwrap();

    assert_eq!(
        banco.versao_do_esquema().unwrap(),
        migracoes::versao_suportada(MIGRACOES)
    );
    let conexao = conexao_crua(&pasta.arquivo_do_banco());
    for tabela in [
        "configuracoes",
        "projetos",
        "conversas",
        "mensagens",
        "memorias",
        "exclusoes",
    ] {
        assert_eq!(
            contar(
                &conexao,
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{tabela}'"
                )
            ),
            1,
            "a tabela {tabela} deve existir"
        );
    }
}

#[test]
fn banco_novo_nao_gera_copia_de_seguranca() {
    let pasta = PastaTemporaria::nova();
    Banco::abrir(&pasta.arquivo_do_banco(), &pasta.pasta_de_copias()).unwrap();

    assert!(copias_na_pasta(&pasta.pasta_de_copias()).is_empty());
}

#[test]
fn migracoes_aplicam_em_banco_de_versao_anterior_sem_perder_dados() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();

    // Banco criado por uma versão anterior do app, que só conhecia a migração 1.
    {
        let antigo = Banco::abrir_com(&arquivo, &[V1], Some(&pasta.pasta_de_copias())).unwrap();
        assert_eq!(antigo.versao_do_esquema().unwrap(), 1);
    }
    let bruta = conexao_crua(&arquivo);
    bruta
        .execute("INSERT INTO itens (nome) VALUES ('antigo')", [])
        .unwrap();
    drop(bruta);

    let novo = Banco::abrir_com(&arquivo, &[V1, V2], Some(&pasta.pasta_de_copias())).unwrap();
    assert_eq!(novo.versao_do_esquema().unwrap(), 2);

    let bruta = conexao_crua(&arquivo);
    let (nome, nota): (String, Option<String>) = bruta
        .query_row("SELECT nome, nota FROM itens", [], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .unwrap();
    assert_eq!((nome.as_str(), nota), ("antigo", None));
    assert_eq!(copias_na_pasta(&pasta.pasta_de_copias()).len(), 1);
}

#[test]
fn migracoes_sao_idempotentes() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
    let p = banco.criar_projeto(&projeto("Fixo")).unwrap();
    drop(banco);

    let antes = esquema(&conexao_crua(&arquivo));
    for _ in 0..3 {
        let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
        assert_eq!(banco.obter_projeto(&p.id).unwrap(), p);
    }
    let depois = esquema(&conexao_crua(&arquivo));

    assert_eq!(antes, depois);
    assert!(
        copias_na_pasta(&pasta.pasta_de_copias()).is_empty(),
        "sem migração pendente, nenhuma cópia deve ser criada"
    );
}

#[test]
fn r27_migracao_que_falha_deixa_o_banco_como_estava_e_mostra_mensagem_amigavel() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    {
        Banco::abrir_com(&arquivo, &[V1], Some(&pasta.pasta_de_copias())).unwrap();
    }
    let bruta = conexao_crua(&arquivo);
    bruta
        .execute("INSERT INTO itens (nome) VALUES ('precioso')", [])
        .unwrap();
    let esquema_antes = esquema(&bruta);
    drop(bruta);

    // A migração 2 é boa e a 3 quebra: nem a 2 pode ficar aplicada.
    let resultado = Banco::abrir_com(
        &arquivo,
        &[V1, V2, V3_QUEBRADA],
        Some(&pasta.pasta_de_copias()),
    );
    let Err(erro) = resultado else {
        panic!("a migração quebrada deveria falhar");
    };

    assert!(matches!(
        erro,
        ErroBanco::MigracaoFalhou {
            versao: 3,
            copia_guardada: true,
            ..
        }
    ));
    let mensagem = erro.mensagem_amigavel();
    assert!(mensagem.contains("continuam como estavam"));
    assert!(mensagem.contains("cópia de segurança"));
    for termo_tecnico in ["tabela_que_nao_existe", "no such table", "SQL", "sqlite"] {
        assert!(
            !mensagem.contains(termo_tecnico),
            "a mensagem não pode vazar texto técnico: {termo_tecnico}"
        );
    }

    let bruta = conexao_crua(&arquivo);
    assert_eq!(esquema(&bruta), esquema_antes, "o esquema não pode mudar");
    assert_eq!(contar(&bruta, "PRAGMA user_version"), 1);
    assert_eq!(contar(&bruta, "SELECT COUNT(*) FROM itens"), 1);
    assert_eq!(copias_na_pasta(&pasta.pasta_de_copias()).len(), 1);
}

#[test]
fn copias_de_seguranca_mantem_so_as_tres_mais_recentes_e_nao_tocam_em_outros_arquivos() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    let copias = pasta.pasta_de_copias();
    fs::create_dir_all(&copias).unwrap();
    let alheio = copias.join("minhas-anotacoes.txt");
    fs::write(&alheio, "não apague").unwrap();

    // Cada rodada acrescenta uma migração nova à lista, então cada abertura migra o banco
    // de uma versão para a seguinte e cria uma cópia de segurança antes.
    for rodada in 1..=6 {
        let lista: Vec<Migracao> = (1..=rodada)
            .map(|versao| Migracao {
                versao,
                nome: "passo",
                sql: if versao == 1 { V1.sql } else { "SELECT 1;" },
            })
            .collect();
        Banco::abrir_com(&arquivo, &lista, Some(&copias)).unwrap();
    }

    let restantes: Vec<PathBuf> = copias_na_pasta(&copias)
        .into_iter()
        .filter(|c| c.extension().is_some_and(|e| e == "db"))
        .collect();
    assert_eq!(restantes.len(), 3);
    assert!(
        alheio.exists(),
        "arquivos que não são cópias nunca são apagados"
    );
}

#[test]
fn banco_de_versao_mais_nova_e_recusado_sem_alterar_nada() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    {
        Banco::abrir_com(&arquivo, &[V1, V2], None).unwrap();
    }
    let esquema_antes = esquema(&conexao_crua(&arquivo));

    let Err(erro) = Banco::abrir_com(&arquivo, &[V1], Some(&pasta.pasta_de_copias())) else {
        panic!("um banco mais novo que o app deveria ser recusado");
    };

    assert!(matches!(
        erro,
        ErroBanco::VersaoMaisNova {
            encontrada: 2,
            suportada: 1
        }
    ));
    assert!(erro.mensagem_amigavel().contains("Atualize o aplicativo"));
    assert_eq!(esquema(&conexao_crua(&arquivo)), esquema_antes);
}

#[test]
fn r26_arquivo_que_nao_e_banco_vira_erro_amigavel_e_nao_e_apagado() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    let conteudo = "isto não é um banco de dados SQLite, é só texto qualquer".repeat(40);
    fs::write(&arquivo, &conteudo).unwrap();

    let Err(erro) = Banco::abrir(&arquivo, &pasta.pasta_de_copias()) else {
        panic!("um arquivo inválido deveria dar erro");
    };

    assert!(matches!(erro, ErroBanco::Indisponivel(_)));
    let mensagem = erro.mensagem_amigavel();
    assert!(mensagem.contains("Seus dados não foram apagados"));
    assert!(!mensagem.to_lowercase().contains("sqlite"));
    assert!(!mensagem.contains("not a database"));
    assert_eq!(fs::read_to_string(&arquivo).unwrap(), conteudo);
}

// ----- Regras dos dados -----

#[test]
fn memoria_global_nao_pertence_a_projeto_e_a_de_projeto_exige_projeto_existente() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
    let p = banco.criar_projeto(&projeto("Com memórias")).unwrap();

    let global = banco.criar_memoria_global("Vale para tudo").unwrap();
    assert_eq!(global.escopo, EscopoMemoria::Global);
    assert_eq!(global.projeto_id, None);

    let do_projeto = banco
        .criar_memoria_de_projeto(&p.id, "Só deste projeto")
        .unwrap();
    assert_eq!(do_projeto.escopo, EscopoMemoria::Projeto);
    assert_eq!(do_projeto.projeto_id.as_deref(), Some(p.id.as_str()));

    // Projeto inexistente é recusado.
    let Err(erro) = banco.criar_memoria_de_projeto(&novo_id(), "órfã") else {
        panic!("memória de projeto inexistente deveria ser recusada");
    };
    assert!(matches!(erro, ErroBanco::Invalido(_)));

    // As combinações impossíveis também são recusadas pelo próprio banco, mesmo por SQL cru.
    let bruta = conexao_crua(&arquivo);
    assert!(
        bruta
            .execute(
                "INSERT INTO memorias (id, escopo, projeto_id, conteudo) VALUES (?1, 'global', ?2, 'x')",
                [novo_id(), p.id.clone()],
            )
            .is_err(),
        "memória global com projeto"
    );
    assert!(
        bruta
            .execute(
                "INSERT INTO memorias (id, escopo, projeto_id, conteudo) VALUES (?1, 'projeto', NULL, 'x')",
                [novo_id()],
            )
            .is_err(),
        "memória de projeto sem projeto"
    );
    drop(bruta);

    // Excluir o projeto leva só as memórias dele. A global fica.
    banco.excluir_projeto(&p.id).unwrap();
    assert!(banco.listar_memorias_do_projeto(&p.id).unwrap().is_empty());
    assert_eq!(banco.listar_memorias_globais().unwrap(), vec![global]);
}

#[test]
fn nenhum_id_e_reaproveitado_depois_de_uma_exclusao() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();

    let p = banco.criar_projeto(&projeto("Efêmero")).unwrap();
    let c = banco.criar_conversa(&p.id, &conversa("Conversa")).unwrap();
    let m = banco
        .adicionar_mensagem(&c.id, Papel::Usuario, "oi", None)
        .unwrap();
    let mem = banco.criar_memoria_de_projeto(&p.id, "lembrar").unwrap();
    let global = banco.criar_memoria_global("global").unwrap();

    banco.excluir_memoria(&global.id).unwrap();
    banco.excluir_projeto(&p.id).unwrap();

    // O registro de exclusões guarda só ID e tipo, inclusive para o que saiu em cascata.
    assert_eq!(
        banco.tipo_da_exclusao(&p.id).unwrap().as_deref(),
        Some("projeto")
    );
    assert_eq!(
        banco.tipo_da_exclusao(&c.id).unwrap().as_deref(),
        Some("conversa")
    );
    assert_eq!(
        banco.tipo_da_exclusao(&m.id).unwrap().as_deref(),
        Some("mensagem")
    );
    assert_eq!(
        banco.tipo_da_exclusao(&mem.id).unwrap().as_deref(),
        Some("memoria")
    );
    assert_eq!(
        banco.tipo_da_exclusao(&global.id).unwrap().as_deref(),
        Some("memoria")
    );

    // Tentar voltar a usar um ID excluído é recusado pelo banco.
    let bruta = conexao_crua(&arquivo);
    for (tabela, id) in [("projetos", &p.id), ("memorias", &global.id)] {
        let sql = if tabela == "projetos" {
            "INSERT INTO projetos (id, nome) VALUES (?1, 'volta')"
        } else {
            "INSERT INTO memorias (id, escopo, conteudo) VALUES (?1, 'global', 'volta')"
        };
        let erro = bruta
            .execute(sql, [id])
            .expect_err("ID excluído não pode voltar");
        assert!(erro.to_string().contains("id_reutilizado"), "{tabela}");
    }
    drop(bruta);

    // Os IDs novos nunca coincidem com os excluídos.
    let excluidos = [&p.id, &c.id, &m.id, &mem.id, &global.id];
    for _ in 0..200 {
        let novo = banco.criar_projeto(&projeto("Novo")).unwrap();
        assert!(!excluidos.contains(&&novo.id));
    }
}

#[test]
fn excluir_projeto_apaga_em_cascata_e_o_resumo_mostra_o_que_sera_apagado() {
    let pasta = PastaTemporaria::nova();
    let banco = Banco::abrir(&pasta.arquivo_do_banco(), &pasta.pasta_de_copias()).unwrap();
    let alvo = banco.criar_projeto(&projeto("Alvo")).unwrap();
    let outro = banco.criar_projeto(&projeto("Outro")).unwrap();

    for titulo in ["a", "b"] {
        let c = banco.criar_conversa(&alvo.id, &conversa(titulo)).unwrap();
        banco
            .adicionar_mensagem(&c.id, Papel::Usuario, "1", None)
            .unwrap();
        banco
            .adicionar_mensagem(&c.id, Papel::Assistente, "2", None)
            .unwrap();
        banco
            .adicionar_mensagem(&c.id, Papel::Usuario, "3", None)
            .unwrap();
    }
    banco.criar_memoria_de_projeto(&alvo.id, "m1").unwrap();
    let c_outro = banco.criar_conversa(&outro.id, &conversa("fica")).unwrap();
    banco
        .adicionar_mensagem(&c_outro.id, Papel::Usuario, "fica", None)
        .unwrap();
    banco.criar_memoria_global("global").unwrap();

    let resumo = banco.resumo_exclusao_projeto(&alvo.id).unwrap();
    assert_eq!(
        (resumo.conversas, resumo.mensagens, resumo.memorias),
        (2, 6, 1)
    );

    banco.excluir_projeto(&alvo.id).unwrap();

    assert!(matches!(
        banco.obter_projeto(&alvo.id),
        Err(ErroBanco::NaoEncontrado)
    ));
    assert!(
        banco
            .listar_conversas_do_projeto(&alvo.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        banco.listar_conversas_do_projeto(&outro.id).unwrap().len(),
        1
    );
    assert_eq!(banco.listar_mensagens(&c_outro.id).unwrap().len(), 1);
    assert_eq!(banco.listar_memorias_globais().unwrap().len(), 1);
    assert!(matches!(
        banco.excluir_projeto(&alvo.id),
        Err(ErroBanco::NaoEncontrado)
    ));
}

#[test]
fn conversa_exige_projeto_existente() {
    let banco = Banco::em_memoria().unwrap();

    let Err(erro) = banco.criar_conversa(&novo_id(), &conversa("sem projeto")) else {
        panic!("conversa sem projeto deveria ser recusada");
    };
    assert!(matches!(erro, ErroBanco::Invalido(_)));
}

#[test]
fn cada_conversa_guarda_o_proprio_modelo_e_os_proprios_parametros() {
    let banco = Banco::em_memoria().unwrap();
    let p = banco.criar_projeto(&projeto("Projeto")).unwrap();
    let a = banco.criar_conversa(&p.id, &conversa("A")).unwrap();
    let b = banco.criar_conversa(&p.id, &conversa("B")).unwrap();

    banco
        .atualizar_conversa(
            &a.id,
            &DadosConversa {
                titulo: "A".into(),
                modelo: Some("qwen3:8b".into()),
                temperature: Some(0.2),
                num_ctx: Some(8192),
                keep_alive: Some("30m".into()),
            },
        )
        .unwrap();

    let a = banco.obter_conversa(&a.id).unwrap();
    assert_eq!(a.modelo.as_deref(), Some("qwen3:8b"));
    assert_eq!(a.temperature, Some(0.2));
    assert_eq!(a.num_ctx, Some(8192));
    assert_eq!(a.keep_alive.as_deref(), Some("30m"));
    assert_eq!(banco.obter_conversa(&b.id).unwrap(), b);
}

#[test]
fn mensagens_ficam_em_ordem_e_substituir_mantem_id_e_posicao() {
    let banco = Banco::em_memoria().unwrap();
    let p = banco.criar_projeto(&projeto("P")).unwrap();
    let c = banco.criar_conversa(&p.id, &conversa("C")).unwrap();
    let outra = banco.criar_conversa(&p.id, &conversa("Outra")).unwrap();

    let m1 = banco
        .adicionar_mensagem(&c.id, Papel::Usuario, "pergunta", None)
        .unwrap();
    let m2 = banco
        .adicionar_mensagem(
            &c.id,
            Papel::Assistente,
            "resposta antiga",
            Some("raciocínio antigo"),
        )
        .unwrap();
    let o1 = banco
        .adicionar_mensagem(&outra.id, Papel::Usuario, "x", None)
        .unwrap();

    assert_eq!((m1.posicao, m2.posicao, o1.posicao), (1, 2, 1));

    let nova = banco
        .substituir_mensagem(&m2.id, "resposta nova", None)
        .unwrap();
    assert_eq!(nova.id, m2.id);
    assert_eq!(nova.posicao, 2);
    assert_eq!(nova.conteudo, "resposta nova");
    assert_eq!(nova.raciocinio, None);

    let todas = banco.listar_mensagens(&c.id).unwrap();
    assert_eq!(todas.len(), 2);
    assert_eq!(todas[0].conteudo, "pergunta");
    assert_eq!(todas[1].conteudo, "resposta nova");
}

#[test]
fn papel_invalido_e_recusado_pelo_banco() {
    let pasta = PastaTemporaria::nova();
    let arquivo = pasta.arquivo_do_banco();
    let banco = Banco::abrir(&arquivo, &pasta.pasta_de_copias()).unwrap();
    let p = banco.criar_projeto(&projeto("P")).unwrap();
    let c = banco.criar_conversa(&p.id, &conversa("C")).unwrap();

    let bruta = conexao_crua(&arquivo);
    let resultado = bruta.execute(
        "INSERT INTO mensagens (id, conversa_id, posicao, papel, conteudo)
         VALUES (?1, ?2, 1, 'system', 'não pode')",
        [novo_id(), c.id],
    );
    assert!(resultado.is_err());
}

#[test]
fn configuracoes_sao_gravadas_e_sobrescritas() {
    let banco = Banco::em_memoria().unwrap();

    assert_eq!(banco.obter_configuracao("tema").unwrap(), None);
    banco.definir_configuracao("tema", "claro").unwrap();
    banco.definir_configuracao("tema", "escuro").unwrap();

    assert_eq!(
        banco.obter_configuracao("tema").unwrap().as_deref(),
        Some("escuro")
    );
}

#[test]
fn todas_as_entidades_tem_id_estavel_e_datas_de_criacao_e_atualizacao() {
    let banco = Banco::em_memoria().unwrap();
    let p = banco.criar_projeto(&projeto("P")).unwrap();

    let iso =
        |texto: &str| texto.len() == 24 && texto.ends_with('Z') && texto.as_bytes()[10] == b'T';
    assert!(!p.id.is_empty());
    assert!(iso(&p.criado_em), "{}", p.criado_em);
    assert!(iso(&p.atualizado_em), "{}", p.atualizado_em);

    std::thread::sleep(std::time::Duration::from_millis(10));
    let editado = banco
        .atualizar_projeto(&p.id, &projeto("P editado"))
        .unwrap();

    assert_eq!(editado.id, p.id);
    assert_eq!(editado.criado_em, p.criado_em);
    assert!(editado.atualizado_em > p.atualizado_em);
}

#[test]
fn atualizar_ou_excluir_o_que_nao_existe_da_erro_de_nao_encontrado() {
    let banco = Banco::em_memoria().unwrap();
    let id = novo_id();

    assert!(matches!(
        banco.atualizar_projeto(&id, &projeto("x")),
        Err(ErroBanco::NaoEncontrado)
    ));
    assert!(matches!(
        banco.excluir_conversa(&id),
        Err(ErroBanco::NaoEncontrado)
    ));
    assert!(matches!(
        banco.excluir_memoria(&id),
        Err(ErroBanco::NaoEncontrado)
    ));
    assert!(matches!(
        banco.substituir_mensagem(&id, "x", None),
        Err(ErroBanco::NaoEncontrado)
    ));
}

#[test]
fn mensagens_amigaveis_nunca_vazam_texto_tecnico() {
    let banco = Banco::em_memoria().unwrap();
    let Err(erro) = banco.criar_memoria_de_projeto(&novo_id(), "órfã") else {
        panic!("deveria falhar");
    };
    // O texto técnico existe, mas só para o log.
    assert!(erro.to_string().to_lowercase().contains("constraint"));

    let todos = [
        erro,
        ErroBanco::Indisponivel("database is locked: C:\\Users\\x\\memoteca.db".into()),
        ErroBanco::SemEspaco,
        ErroBanco::Interno("SQLITE_IOERR".into()),
        ErroBanco::NaoEncontrado,
    ];
    for erro in todos {
        let mensagem = erro.mensagem_amigavel();
        for proibido in [
            "constraint",
            "FOREIGN KEY",
            "SQLITE",
            "database",
            "C:\\",
            ".db",
        ] {
            assert!(
                !mensagem.contains(proibido),
                "`{mensagem}` contém texto técnico: {proibido}"
            );
        }
    }
}
