//! Testes do log local (E14): onde os erros técnicos ficam, no lugar de aparecer para o usuário.

use std::fs;

use memoteca_lib::banco::id::novo_id;
use memoteca_lib::banco::intercambio::Backup;
use memoteca_lib::log::{ARQUIVOS_MANTIDOS, TAMANHO_MAXIMO, iniciar, pasta_do_log};
use memoteca_lib::registrar;

#[test]
fn o_log_vai_para_logs_memoteca_log_na_pasta_de_dados_com_a_data_e_so_detalhe_tecnico() {
    let dados = std::env::temp_dir().join(format!("memoteca-log-{}", novo_id()));
    fs::create_dir_all(&dados).unwrap();
    iniciar(&pasta_do_log(&dados));

    registrar!("a importação falhou: {}", "banco indisponível");

    let arquivo = dados.join("logs").join("memoteca.log");
    let conteudo = fs::read_to_string(&arquivo).unwrap();
    let linha = conteudo.lines().next().unwrap();
    // `2026-11-11T00:12:03.123Z ERRO a importação falhou: banco indisponível`
    let (instante, resto) = linha.split_once(' ').unwrap();
    assert_eq!(instante.len(), 24, "{linha}");
    assert!(instante.ends_with('Z') && instante.contains('T'), "{linha}");
    assert_eq!(resto, "ERRO a importação falhou: banco indisponível");
    let _ = fs::remove_dir_all(&dados);
}

#[test]
fn a_retencao_e_de_tres_arquivos_de_ate_um_megabyte() {
    assert_eq!(TAMANHO_MAXIMO, 1024 * 1024);
    assert_eq!(ARQUIVOS_MANTIDOS, 3);
}

/// O detalhe técnico que vai para o log nunca cita o conteúdo do arquivo do usuário. O serde
/// costuma citar o valor que não bateu com o tipo, e um backup tem as conversas inteiras.
#[test]
fn o_detalhe_de_um_backup_recusado_nao_carrega_o_conteudo_das_conversas() {
    let segredo = "SEGREDO-DO-USUARIO-NUNCA-NO-LOG";
    let corpos = [
        // Valor do tipo errado, com o segredo logo ao lado.
        format!(
            r#"{{"formato":"memoteca-backup","versao":1,"exportadoEm":"2026-11-10T00:00:00.000Z","app":"0.1.0",
               "projetos":[{{"id":"0192f0a0-0000-7000-8000-000000000001","nome":42,"systemPrompt":"{segredo}",
               "modeloPadrao":null,"criadoEm":"x","atualizadoEm":"x"}}],"conversas":[],"mensagens":[],"memorias":[]}}"#
        ),
        // JSON quebrado no meio de um texto do usuário.
        format!(r#"{{"formato":"memoteca-backup","versao":1,"projetos":[{{"nome":"{segredo}"#),
        // Texto de memória com o segredo e uma estrutura que não fecha.
        format!(r#"{{"formato":"memoteca-backup","versao":1,"memorias":"{segredo}"}}"#),
    ];

    for corpo in corpos {
        let erro = Backup::ler(corpo.as_bytes()).unwrap_err();

        assert!(!erro.to_string().contains(segredo), "{erro}");
        assert!(!format!("{erro:?}").contains(segredo), "{erro:?}");
        assert!(!erro.mensagem_amigavel().contains(segredo));
    }
}
