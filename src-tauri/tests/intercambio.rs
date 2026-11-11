//! Testes de aceite da E13: exportar e importar. Os critérios de falha do plano levam
//! "(falha)" no nome, e os cenários do plano de robustez levam o identificador `Rnn`.
//! (O R29, falha no meio da gravação, está nos testes do próprio módulo `banco::intercambio`,
//! que precisa injetar a falha dentro da transação.)

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::banco::intercambio::{Backup, ErroDeBackup, FORMATO, VERSAO_DO_FORMATO};
use memoteca_lib::banco::{Banco, DadosConversa, DadosProjeto, EstadoMensagem, Papel};
use memoteca_lib::intercambio::{
    ErroDeExportacao, exportar_para, gravar_atomico, gravar_atomico_com, ler_backup,
    ler_backup_limitado,
};
use serde_json::{Value, json};

// ----- Apoio -----

struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    fn nova() -> Self {
        let pasta = std::env::temp_dir().join(format!("memoteca-intercambio-{}", novo_id()));
        fs::create_dir_all(&pasta).unwrap();
        Self(pasta)
    }

    fn arquivo(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }

    /// Nomes de tudo o que há na pasta.
    fn nomes(&self) -> Vec<String> {
        let mut nomes: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        nomes.sort();
        nomes
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        // Arquivos somente leitura precisam perder o atributo antes de sair.
        if let Ok(entradas) = fs::read_dir(&self.0) {
            for entrada in entradas.flatten() {
                if let Ok(meta) = entrada.metadata() {
                    let mut permissoes = meta.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    permissoes.set_readonly(false);
                    let _ = fs::set_permissions(entrada.path(), permissoes);
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Os dados de um banco, sem a data do backup (que muda a cada exportação).
fn conteudo(banco: &Banco) -> Value {
    let mut valor = serde_json::to_value(banco.montar_backup().unwrap()).unwrap();
    valor.as_object_mut().unwrap().remove("exportadoEm");
    valor
}

struct Ids {
    projeto: String,
    conversa: String,
    mensagem_do_usuario: String,
    mensagem_do_assistente: String,
    memoria_global: String,
    memoria_do_projeto: String,
}

/// Um banco com um projeto, uma conversa de duas mensagens (uma com raciocínio e outra
/// interrompida) e duas memórias, com parâmetros que fogem do padrão.
fn banco_cheio() -> (Banco, Ids) {
    let banco = Banco::em_memoria().unwrap();
    let projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "Estudos de Rust".into(),
            system_prompt: "Responda em português.\nSeja breve.".into(),
            modelo_padrao: Some("qwen3:8b".into()),
        })
        .unwrap();
    let conversa = banco.criar_conversa_nova(&projeto.id).unwrap();
    banco
        .atualizar_conversa(
            &conversa.id,
            &DadosConversa {
                titulo: "Ownership e empréstimos".into(),
                modelo: Some("llama3.2:3b".into()),
                temperature: Some(1.3),
                num_ctx: Some(16_384),
                keep_alive: Some("-1".into()),
            },
        )
        .unwrap();
    let pergunta = banco
        .adicionar_mensagem(&conversa.id, Papel::Usuario, "O que é ownership? 🦀", None)
        .unwrap();
    let resposta = banco
        .adicionar_mensagem_com_estado(
            &conversa.id,
            Papel::Assistente,
            "É o modelo de memória do Rust, com\nquebras e “aspas”.",
            Some("Vou explicar."),
            EstadoMensagem::Interrompida,
        )
        .unwrap();
    let global = banco.criar_memoria_global("Mora em Recife").unwrap();
    let do_projeto = banco
        .criar_memoria_de_projeto(&projeto.id, "Usa Rust 2024")
        .unwrap();
    (
        banco,
        Ids {
            projeto: projeto.id,
            conversa: conversa.id,
            mensagem_do_usuario: pergunta.id,
            mensagem_do_assistente: resposta.id,
            memoria_global: global.id,
            memoria_do_projeto: do_projeto.id,
        },
    )
}

fn exportar(banco: &Banco, pasta: &PastaTemporaria, nome: &str) -> PathBuf {
    let destino = pasta.arquivo(nome);
    exportar_para(banco, &destino).unwrap();
    destino
}

fn contar(banco: &Banco, ids: &[&str]) -> usize {
    // Quantos destes IDs existem, em qualquer tabela.
    let backup = banco.montar_backup().unwrap();
    let todos: Vec<&str> = backup
        .projetos
        .iter()
        .map(|p| p.id.as_str())
        .chain(backup.conversas.iter().map(|c| c.id.as_str()))
        .chain(backup.mensagens.iter().map(|m| m.id.as_str()))
        .chain(backup.memorias.iter().map(|m| m.id.as_str()))
        .collect();
    ids.iter().filter(|id| todos.contains(id)).count()
}

// ----- Exportar e importar em banco vazio: idêntico, com os mesmos IDs -----

#[test]
fn exportar_e_importar_num_banco_vazio_reproduz_tudo_identico_com_os_mesmos_ids() {
    let pasta = PastaTemporaria::nova();
    let (origem, ids) = banco_cheio();
    let arquivo = exportar(&origem, &pasta, "backup.json");

    let destino = Banco::em_memoria().unwrap();
    let backup = ler_backup(&arquivo).unwrap();
    let resumo = destino.importar_backup(&backup).unwrap();

    assert_eq!(conteudo(&destino), conteudo(&origem));
    assert_eq!(resumo.projetos.novos, 1);
    assert_eq!(resumo.conversas.novos, 1);
    assert_eq!(resumo.mensagens.novos, 2);
    assert_eq!(resumo.memorias.novos, 2);
    // Os IDs são os mesmos, e os detalhes que costumam se perder também.
    let importado = destino.obter_conversa(&ids.conversa).unwrap();
    assert_eq!(importado.titulo, "Ownership e empréstimos");
    assert_eq!(importado.temperature, Some(1.3));
    assert_eq!(importado.num_ctx, Some(16_384));
    assert_eq!(importado.keep_alive.as_deref(), Some("-1"));
    let mensagens = destino.listar_mensagens(&ids.conversa).unwrap();
    assert_eq!(mensagens[0].id, ids.mensagem_do_usuario);
    assert_eq!(mensagens[0].conteudo, "O que é ownership? 🦀");
    assert_eq!(mensagens[1].id, ids.mensagem_do_assistente);
    assert_eq!(mensagens[1].raciocinio.as_deref(), Some("Vou explicar."));
    assert_eq!(mensagens[1].estado, EstadoMensagem::Interrompida);
    assert_eq!(
        destino.obter_memoria(&ids.memoria_global).unwrap().conteudo,
        "Mora em Recife"
    );
    assert_eq!(
        destino
            .obter_memoria(&ids.memoria_do_projeto)
            .unwrap()
            .projeto_id
            .as_deref(),
        Some(ids.projeto.as_str())
    );
    // As datas também vêm do arquivo, e não de "agora".
    assert_eq!(
        destino.obter_projeto(&ids.projeto).unwrap().criado_em,
        origem.obter_projeto(&ids.projeto).unwrap().criado_em
    );
}

#[test]
fn exportar_um_banco_vazio_gera_um_backup_valido_que_importa_sem_nada() {
    let pasta = PastaTemporaria::nova();
    let vazio = Banco::em_memoria().unwrap();
    let arquivo = exportar(&vazio, &pasta, "vazio.json");

    let backup = ler_backup(&arquivo).unwrap();
    let resumo = Banco::em_memoria()
        .unwrap()
        .importar_backup(&backup)
        .unwrap();

    assert!(backup.projetos.is_empty() && backup.mensagens.is_empty());
    assert_eq!(resumo.total_de_novos(), 0);
}

#[test]
fn o_arquivo_exportado_informa_a_versao_do_formato() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();
    let arquivo = exportar(&banco, &pasta, "backup.json");

    let texto = fs::read_to_string(&arquivo).unwrap();
    let json: Value = serde_json::from_str(&texto).unwrap();

    assert_eq!(json["formato"], FORMATO);
    assert_eq!(json["versao"], VERSAO_DO_FORMATO);
    assert_eq!(json["versao"], 1);
    assert!(json["exportadoEm"].as_str().unwrap().ends_with('Z'));
    assert!(json["app"].as_str().is_some());
    for chave in ["projetos", "conversas", "mensagens", "memorias"] {
        assert!(json[chave].is_array(), "{chave}");
    }
    // As configurações do app não entram no backup.
    assert!(json.get("configuracoes").is_none());
}

#[test]
fn exportar_nao_altera_os_dados_e_so_deixa_o_arquivo_pedido() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();
    let antes = conteudo(&banco);

    exportar(&banco, &pasta, "backup.json");
    exportar(&banco, &pasta, "backup.json");

    assert_eq!(conteudo(&banco), antes);
    assert_eq!(pasta.nomes(), ["backup.json"], "nenhum temporário sobrando");
}

// ----- Arquivo inválido, truncado ou de versão desconhecida (falha) -----

#[test]
fn r28_arquivo_invalido_truncado_ou_de_versao_desconhecida_e_recusado_sem_alterar_nada() {
    let pasta = PastaTemporaria::nova();
    let (origem, _) = banco_cheio();
    let valido = fs::read(exportar(&origem, &pasta, "valido.json")).unwrap();

    let (destino, _) = banco_cheio();
    let antes = conteudo(&destino);

    let mut versao_nova: Value = serde_json::from_slice(&valido).unwrap();
    versao_nova["versao"] = json!(99);
    let mut sem_marca: Value = serde_json::from_slice(&valido).unwrap();
    sem_marca.as_object_mut().unwrap().remove("formato");
    let mut referencia_solta: Value = serde_json::from_slice(&valido).unwrap();
    referencia_solta["conversas"][0]["projetoId"] = json!(novo_id());
    let mut campo_faltando: Value = serde_json::from_slice(&valido).unwrap();
    campo_faltando["projetos"][0]
        .as_object_mut()
        .unwrap()
        .remove("nome");

    let casos: Vec<(&str, Vec<u8>)> = vec![
        ("vazio", Vec::new()),
        ("lixo", b"\x00\x01\x02 nao sou json".to_vec()),
        ("texto qualquer", "Olá, eu sou uma lista de compras".into()),
        ("truncado na metade", valido[..valido.len() / 2].to_vec()),
        ("truncado quase no fim", valido[..valido.len() - 3].to_vec()),
        (
            "JSON de outro programa",
            br#"{"nome":"outro","itens":[1,2]}"#.to_vec(),
        ),
        (
            "versão desconhecida",
            serde_json::to_vec(&versao_nova).unwrap(),
        ),
        (
            "sem a marca do formato",
            serde_json::to_vec(&sem_marca).unwrap(),
        ),
        (
            "conversa de um projeto que não está no arquivo",
            serde_json::to_vec(&referencia_solta).unwrap(),
        ),
        (
            "projeto sem nome",
            serde_json::to_vec(&campo_faltando).unwrap(),
        ),
    ];
    for (nome, bytes) in casos {
        let arquivo = pasta.arquivo("ruim.json");
        fs::write(&arquivo, &bytes).unwrap();

        let erro = ler_backup(&arquivo).expect_err(nome);

        let amigavel = erro.mensagem_amigavel();
        assert!(amigavel.contains("Nada foi"), "{nome}: {amigavel}");
        for tecnico in ["expected", "EOF", "line 1", "serde", "missing field"] {
            assert!(!amigavel.contains(tecnico), "{nome}: {amigavel}");
        }
        // Como a leitura recusa antes de qualquer gravação, o banco nem foi tocado.
        assert_eq!(conteudo(&destino), antes, "{nome}");
    }
}

#[test]
fn versao_desconhecida_diz_qual_e_o_formato_e_pede_para_atualizar() {
    let pasta = PastaTemporaria::nova();
    let (origem, _) = banco_cheio();
    let mut json: Value =
        serde_json::from_slice(&fs::read(exportar(&origem, &pasta, "a.json")).unwrap()).unwrap();
    json["versao"] = json!(7);
    fs::write(pasta.arquivo("b.json"), serde_json::to_vec(&json).unwrap()).unwrap();

    let erro = ler_backup(&pasta.arquivo("b.json")).unwrap_err();

    assert_eq!(erro, ErroDeBackup::VersaoDesconhecida(Some(7)));
    assert!(erro.mensagem_amigavel().contains("formato 7"));
    assert!(erro.mensagem_amigavel().contains("Atualize"));
}

#[test]
fn arquivo_que_nao_existe_ou_grande_demais_tem_mensagem_amigavel() {
    let pasta = PastaTemporaria::nova();

    let ausente = ler_backup(&pasta.arquivo("nao-existe.json")).unwrap_err();
    assert!(matches!(ausente, ErroDeBackup::ArquivoIlegivel(_)));
    assert!(ausente.mensagem_amigavel().contains("Nada foi alterado"));
    assert!(!ausente.mensagem_amigavel().contains("nao-existe"));

    fs::write(pasta.arquivo("grande.json"), "x".repeat(100)).unwrap();
    let grande = ler_backup_limitado(&pasta.arquivo("grande.json"), 10).unwrap_err();
    assert_eq!(grande, ErroDeBackup::ArquivoGrandeDemais);
}

// ----- Política de importação: mesclar, sem duplicar -----

#[test]
fn importar_sobre_os_mesmos_dados_nao_duplica_nem_muda_nada() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();
    let arquivo = exportar(&banco, &pasta, "backup.json");
    let antes = conteudo(&banco);

    let resumo = banco
        .importar_backup(&ler_backup(&arquivo).unwrap())
        .unwrap();

    assert_eq!(conteudo(&banco), antes, "nada duplicado nem alterado");
    assert_eq!(resumo.total_de_novos(), 0);
    assert_eq!(resumo.projetos.ja_existem, 1);
    assert_eq!(resumo.conversas.ja_existem, 1);
    assert_eq!(resumo.mensagens.ja_existem, 2);
    assert_eq!(resumo.memorias.ja_existem, 2);
    assert_eq!(resumo.projetos.ignorados + resumo.memorias.ignorados, 0);
}

#[test]
fn mesclar_mantem_o_que_ja_existe_como_esta_e_adiciona_so_o_que_falta() {
    let pasta = PastaTemporaria::nova();
    let (banco, ids) = banco_cheio();
    let arquivo = exportar(&banco, &pasta, "backup.json");

    // Depois do backup, o usuário muda coisas aqui e cria outras.
    banco
        .atualizar_projeto(
            &ids.projeto,
            &DadosProjeto {
                nome: "Rust (renomeado depois do backup)".into(),
                system_prompt: "Outro prompt.".into(),
                modelo_padrao: None,
            },
        )
        .unwrap();
    banco
        .atualizar_memoria(&ids.memoria_global, "Mora em Olinda")
        .unwrap();
    let nova_mensagem = banco
        .adicionar_mensagem(&ids.conversa, Papel::Usuario, "Mensagem posterior", None)
        .unwrap();
    let outro_projeto = banco
        .criar_projeto(&DadosProjeto {
            nome: "Só existe aqui".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let antes = conteudo(&banco);

    let resumo = banco
        .importar_backup(&ler_backup(&arquivo).unwrap())
        .unwrap();

    // Tudo igual: o backup não sobrescreve nem some com o que é mais novo aqui.
    assert_eq!(conteudo(&banco), antes);
    assert_eq!(
        banco.obter_projeto(&ids.projeto).unwrap().nome,
        "Rust (renomeado depois do backup)"
    );
    assert_eq!(
        banco.obter_memoria(&ids.memoria_global).unwrap().conteudo,
        "Mora em Olinda"
    );
    assert_eq!(banco.listar_mensagens(&ids.conversa).unwrap().len(), 3);
    assert_eq!(contar(&banco, &[&nova_mensagem.id, &outro_projeto.id]), 2);
    assert_eq!(resumo.total_de_novos(), 0);
    assert_eq!(resumo.mensagens.ja_existem, 2);
}

#[test]
fn mesclar_traz_o_que_nao_existe_aqui_e_mantem_o_resto() {
    let pasta = PastaTemporaria::nova();
    let (origem, ids) = banco_cheio();
    let arquivo = exportar(&origem, &pasta, "backup.json");

    // O destino tem outro projeto e já tem a memória global do backup (mesmo ID).
    let destino = Banco::em_memoria().unwrap();
    let existente = destino
        .criar_projeto(&DadosProjeto {
            nome: "Já estava aqui".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let backup = ler_backup(&arquivo).unwrap();

    let resumo = destino.importar_backup(&backup).unwrap();

    assert_eq!(resumo.projetos.novos, 1);
    assert_eq!(destino.listar_projetos().unwrap().len(), 2);
    assert!(destino.obter_projeto(&existente.id).is_ok());
    assert!(destino.obter_projeto(&ids.projeto).is_ok());
    assert_eq!(destino.listar_mensagens(&ids.conversa).unwrap().len(), 2);
}

#[test]
fn importar_duas_vezes_o_mesmo_backup_da_o_mesmo_resultado_de_uma() {
    let pasta = PastaTemporaria::nova();
    let (origem, _) = banco_cheio();
    let backup = ler_backup(&exportar(&origem, &pasta, "backup.json")).unwrap();
    let destino = Banco::em_memoria().unwrap();

    destino.importar_backup(&backup).unwrap();
    let depois_da_primeira = conteudo(&destino);
    let segunda = destino.importar_backup(&backup).unwrap();

    assert_eq!(conteudo(&destino), depois_da_primeira);
    assert_eq!(segunda.total_de_novos(), 0);
}

#[test]
fn o_que_o_usuario_excluiu_aqui_e_ignorado_e_nao_volta() {
    let pasta = PastaTemporaria::nova();
    let (banco, ids) = banco_cheio();
    let outro = banco
        .criar_projeto(&DadosProjeto {
            nome: "Outro".into(),
            system_prompt: String::new(),
            modelo_padrao: None,
        })
        .unwrap();
    let memoria_do_outro = banco
        .criar_memoria_de_projeto(&outro.id, "do outro")
        .unwrap();
    let arquivo = exportar(&banco, &pasta, "backup.json");

    // Depois do backup, o usuário exclui o primeiro projeto (e, em cascata, a conversa, as
    // mensagens e a memória dele) e uma memória global.
    banco.excluir_projeto(&ids.projeto).unwrap();
    banco.excluir_memoria(&ids.memoria_global).unwrap();
    let antes = conteudo(&banco);

    let resumo = banco
        .importar_backup(&ler_backup(&arquivo).unwrap())
        .unwrap();

    // Nada do que foi excluído volta, e o resto continua como está.
    assert_eq!(conteudo(&banco), antes);
    assert_eq!(
        contar(
            &banco,
            &[
                &ids.projeto,
                &ids.conversa,
                &ids.mensagem_do_usuario,
                &ids.mensagem_do_assistente,
                &ids.memoria_global,
                &ids.memoria_do_projeto,
            ]
        ),
        0
    );
    assert_eq!(contar(&banco, &[&outro.id, &memoria_do_outro.id]), 2);
    assert_eq!(resumo.projetos.ignorados, 1);
    assert_eq!(resumo.conversas.ignorados, 1);
    assert_eq!(resumo.mensagens.ignorados, 2);
    assert_eq!(resumo.memorias.ignorados, 2);
    assert_eq!(resumo.projetos.ja_existem, 1);
    assert_eq!(resumo.total_de_novos(), 0);
}

#[test]
fn memoria_global_excluida_aqui_e_ignorada_mas_o_resto_do_backup_entra() {
    let pasta = PastaTemporaria::nova();
    let (origem, ids) = banco_cheio();
    let arquivo = exportar(&origem, &pasta, "backup.json");
    // Outro computador, que já teve a memória global e a excluiu.
    let destino = Banco::em_memoria().unwrap();
    let backup = ler_backup(&arquivo).unwrap();
    destino.importar_backup(&backup).unwrap();
    destino.excluir_memoria(&ids.memoria_global).unwrap();

    let resumo = destino.importar_backup(&backup).unwrap();

    assert_eq!(resumo.memorias.ignorados, 1);
    assert_eq!(resumo.memorias.ja_existem, 1);
    assert_eq!(contar(&destino, &[&ids.memoria_global]), 0);
}

#[test]
fn a_analise_conta_o_mesmo_que_a_importacao_e_nao_grava_nada() {
    let pasta = PastaTemporaria::nova();
    let (origem, _) = banco_cheio();
    let backup = ler_backup(&exportar(&origem, &pasta, "backup.json")).unwrap();
    let destino = Banco::em_memoria().unwrap();

    let previsto = destino.analisar_backup(&backup).unwrap();

    assert_eq!(conteudo(&destino), conteudo(&Banco::em_memoria().unwrap()));
    assert_eq!(previsto, destino.importar_backup(&backup).unwrap());
}

// ----- R30: exportar sem permissão ou sem espaço -----

#[test]
fn r30_destino_numa_pasta_que_nao_existe_da_erro_amigavel_e_nao_deixa_arquivo() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();

    let erro = exportar_para(&banco, &pasta.arquivo("nao-existe/backup.json")).unwrap_err();

    assert_eq!(erro, ErroDeExportacao::DestinoInvalido);
    assert!(
        erro.mensagem_amigavel()
            .contains("Nenhum arquivo foi criado")
    );
    assert!(pasta.nomes().is_empty());
}

#[test]
fn r30_destino_somente_leitura_da_erro_amigavel_e_o_arquivo_antigo_fica_intacto() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();
    let destino = pasta.arquivo("backup.json");
    fs::write(&destino, "backup antigo").unwrap();
    let mut permissoes = fs::metadata(&destino).unwrap().permissions();
    permissoes.set_readonly(true);
    fs::set_permissions(&destino, permissoes).unwrap();

    let erro = exportar_para(&banco, &destino).unwrap_err();

    assert_eq!(erro, ErroDeExportacao::SemPermissao);
    let amigavel = erro.mensagem_amigavel();
    assert!(amigavel.contains("permissão") && amigavel.contains("Nenhum arquivo foi criado"));
    assert_eq!(fs::read_to_string(&destino).unwrap(), "backup antigo");
    assert_eq!(pasta.nomes(), ["backup.json"], "sem temporário sobrando");
}

#[test]
fn r30_disco_cheio_no_meio_da_gravacao_nao_deixa_arquivo_parcial_nem_estraga_o_antigo() {
    let pasta = PastaTemporaria::nova();
    let destino = pasta.arquivo("backup.json");
    fs::write(&destino, "backup antigo").unwrap();

    let erro = gravar_atomico_com(&destino, |arquivo| {
        arquivo.write_all(b"metade do backup novo")?;
        Err(io::Error::from(io::ErrorKind::StorageFull))
    })
    .unwrap_err();

    assert_eq!(erro, ErroDeExportacao::SemEspaco);
    assert!(erro.mensagem_amigavel().contains("espaço"));
    assert_eq!(fs::read_to_string(&destino).unwrap(), "backup antigo");
    assert_eq!(pasta.nomes(), ["backup.json"]);

    // Sem arquivo antigo, também não sobra nada.
    let outro = pasta.arquivo("novo.json");
    let erro = gravar_atomico_com(&outro, |arquivo| {
        arquivo.write_all(b"parcial")?;
        Err(io::Error::from(io::ErrorKind::StorageFull))
    })
    .unwrap_err();
    assert_eq!(erro, ErroDeExportacao::SemEspaco);
    assert!(!Path::new(&outro).exists());
    assert_eq!(pasta.nomes(), ["backup.json"]);
}

#[test]
fn gravar_substitui_um_backup_antigo_por_inteiro() {
    let pasta = PastaTemporaria::nova();
    let destino = pasta.arquivo("backup.json");
    fs::write(&destino, "x".repeat(10_000)).unwrap();

    gravar_atomico(&destino, b"curto").unwrap();

    assert_eq!(fs::read_to_string(&destino).unwrap(), "curto");
    assert_eq!(pasta.nomes(), ["backup.json"]);
}

#[test]
fn o_backup_lido_do_arquivo_e_igual_ao_montado() {
    let pasta = PastaTemporaria::nova();
    let (banco, _) = banco_cheio();
    let arquivo = exportar(&banco, &pasta, "backup.json");

    let lido: Backup = ler_backup(&arquivo).unwrap();

    let mut montado = banco.montar_backup().unwrap();
    montado.exportado_em = lido.exportado_em.clone();
    assert_eq!(lido, montado);
}
