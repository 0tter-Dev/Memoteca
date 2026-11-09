//! Migrações versionadas do banco.
//!
//! A versão do esquema fica em `PRAGMA user_version`. Cada migração é um arquivo `.sql`
//! numerado em `migracoes/`. Todas as migrações pendentes rodam **numa única transação**:
//! se qualquer uma falhar, nada é aplicado e o banco continua como estava. Antes de migrar
//! um banco que já existe, o executor faz uma cópia de segurança.

use std::path::Path;

use rusqlite::Connection;

use super::backup;
use super::erro::ErroBanco;

pub struct Migracao {
    /// Número da versão que o banco passa a ter depois desta migração (1, 2, 3...).
    pub versao: u32,
    pub nome: &'static str,
    /// SQL da migração. Roda dentro de uma transação: sem BEGIN, COMMIT nem PRAGMA.
    pub sql: &'static str,
}

/// Migrações do app, em ordem. Para mudar o esquema, acrescente uma nova no fim. Nunca
/// edite uma migração que já foi publicada.
pub const MIGRACOES: &[Migracao] = &[
    Migracao {
        versao: 1,
        nome: "inicial",
        sql: include_str!("migracoes/0001_inicial.sql"),
    },
    Migracao {
        versao: 2,
        nome: "estado da mensagem",
        sql: include_str!("migracoes/0002_estado_da_mensagem.sql"),
    },
];

/// Maior versão que este app conhece.
pub fn versao_suportada(migracoes: &[Migracao]) -> u32 {
    migracoes.last().map_or(0, |migracao| migracao.versao)
}

/// Versão do esquema gravada no banco (0 para um banco novo).
pub fn versao_do_banco(conexao: &Connection) -> Result<u32, ErroBanco> {
    let versao: i64 = conexao.pragma_query_value(None, "user_version", |linha| linha.get(0))?;
    u32::try_from(versao).map_err(|_| ErroBanco::Interno(format!("versão inválida: {versao}")))
}

/// Aplica as migrações pendentes. Rodar de novo, sem migrações novas, não altera nada.
///
/// Quando já existe um banco com dados (versão maior que 0) e há migração pendente, uma
/// cópia de segurança é criada em `pasta_backups` antes de qualquer alteração.
pub fn aplicar(
    conexao: &mut Connection,
    migracoes: &[Migracao],
    pasta_backups: Option<&Path>,
) -> Result<(), ErroBanco> {
    let suportada = versao_suportada(migracoes);
    let atual = versao_do_banco(conexao)?;

    if atual > suportada {
        return Err(ErroBanco::VersaoMaisNova {
            encontrada: atual,
            suportada,
        });
    }

    let pendentes: Vec<&Migracao> = migracoes
        .iter()
        .filter(|migracao| migracao.versao > atual)
        .collect();
    if pendentes.is_empty() {
        return Ok(());
    }

    let mut copia_guardada = false;
    if atual > 0
        && let Some(pasta) = pasta_backups
    {
        backup::criar(conexao, pasta, atual)?;
        copia_guardada = true;
    }

    let falha = |versao: u32| {
        move |erro: rusqlite::Error| ErroBanco::MigracaoFalhou {
            versao,
            detalhe: erro.to_string(),
            copia_guardada,
        }
    };

    // Descartar a transação sem `commit` desfaz tudo.
    let transacao = conexao.transaction().map_err(falha(pendentes[0].versao))?;
    for migracao in &pendentes {
        transacao
            .execute_batch(migracao.sql)
            .map_err(falha(migracao.versao))?;
        transacao
            .pragma_update(None, "user_version", migracao.versao)
            .map_err(falha(migracao.versao))?;
    }
    let ultima = pendentes[pendentes.len() - 1].versao;
    transacao.commit().map_err(falha(ultima))?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_migracoes_do_app_sao_sequenciais_a_partir_de_um() {
        for (indice, migracao) in MIGRACOES.iter().enumerate() {
            assert_eq!(
                migracao.versao as usize,
                indice + 1,
                "a migração `{}` está fora de ordem",
                migracao.nome
            );
            assert!(!migracao.sql.trim().is_empty());
        }
    }
}
