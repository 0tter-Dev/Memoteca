//! Operações de dados sobre `Banco`.

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};

use super::Banco;
use super::erro::ErroBanco;
use super::id::novo_id;
use super::modelos::{
    Conversa, DadosConversa, DadosProjeto, EscopoMemoria, Memoria, Mensagem, Papel, Projeto,
    ResumoExclusaoProjeto,
};

type Resultado<T> = Result<T, ErroBanco>;

/// Instante atual em SQL, no mesmo formato dos valores padrão das colunas de data.
const AGORA: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

fn contagem(valor: i64) -> u64 {
    u64::try_from(valor).unwrap_or_default()
}

fn projeto_da_linha(linha: &Row<'_>) -> rusqlite::Result<Projeto> {
    Ok(Projeto {
        id: linha.get("id")?,
        nome: linha.get("nome")?,
        system_prompt: linha.get("system_prompt")?,
        modelo_padrao: linha.get("modelo_padrao")?,
        criado_em: linha.get("criado_em")?,
        atualizado_em: linha.get("atualizado_em")?,
    })
}

fn conversa_da_linha(linha: &Row<'_>) -> rusqlite::Result<Conversa> {
    Ok(Conversa {
        id: linha.get("id")?,
        projeto_id: linha.get("projeto_id")?,
        titulo: linha.get("titulo")?,
        modelo: linha.get("modelo")?,
        temperature: linha.get("temperature")?,
        num_ctx: linha.get("num_ctx")?,
        keep_alive: linha.get("keep_alive")?,
        criado_em: linha.get("criado_em")?,
        atualizado_em: linha.get("atualizado_em")?,
    })
}

fn mensagem_da_linha(linha: &Row<'_>) -> rusqlite::Result<Mensagem> {
    let papel: String = linha.get("papel")?;
    Ok(Mensagem {
        id: linha.get("id")?,
        conversa_id: linha.get("conversa_id")?,
        posicao: linha.get("posicao")?,
        papel: Papel::de_texto(&papel)
            .ok_or_else(|| rusqlite::Error::InvalidColumnType(0, "papel".into(), Type::Text))?,
        conteudo: linha.get("conteudo")?,
        raciocinio: linha.get("raciocinio")?,
        criado_em: linha.get("criado_em")?,
        atualizado_em: linha.get("atualizado_em")?,
    })
}

fn memoria_da_linha(linha: &Row<'_>) -> rusqlite::Result<Memoria> {
    let escopo: String = linha.get("escopo")?;
    Ok(Memoria {
        id: linha.get("id")?,
        escopo: match escopo.as_str() {
            "global" => EscopoMemoria::Global,
            "projeto" => EscopoMemoria::Projeto,
            _ => {
                return Err(rusqlite::Error::InvalidColumnType(
                    0,
                    "escopo".into(),
                    Type::Text,
                ));
            }
        },
        projeto_id: linha.get("projeto_id")?,
        conteudo: linha.get("conteudo")?,
        criado_em: linha.get("criado_em")?,
        atualizado_em: linha.get("atualizado_em")?,
    })
}

impl Banco {
    // ----- Configurações -----

    pub fn definir_configuracao(&self, chave: &str, valor: &str) -> Resultado<()> {
        self.conexao.execute(
            &format!(
                "INSERT INTO configuracoes (chave, valor) VALUES (?1, ?2)
                 ON CONFLICT (chave) DO UPDATE
                 SET valor = excluded.valor, atualizado_em = {AGORA}"
            ),
            params![chave, valor],
        )?;
        Ok(())
    }

    pub fn obter_configuracao(&self, chave: &str) -> Resultado<Option<String>> {
        Ok(self
            .conexao
            .query_row(
                "SELECT valor FROM configuracoes WHERE chave = ?1",
                [chave],
                |linha| linha.get(0),
            )
            .optional()?)
    }

    // ----- Projetos -----

    pub fn criar_projeto(&self, dados: &DadosProjeto) -> Resultado<Projeto> {
        let id = novo_id();
        self.conexao.execute(
            "INSERT INTO projetos (id, nome, system_prompt, modelo_padrao)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, dados.nome, dados.system_prompt, dados.modelo_padrao],
        )?;
        self.obter_projeto(&id)
    }

    pub fn obter_projeto(&self, id: &str) -> Resultado<Projeto> {
        Ok(self.conexao.query_row(
            "SELECT * FROM projetos WHERE id = ?1",
            [id],
            projeto_da_linha,
        )?)
    }

    pub fn listar_projetos(&self) -> Resultado<Vec<Projeto>> {
        let mut consulta = self
            .conexao
            .prepare("SELECT * FROM projetos ORDER BY criado_em, id")?;
        let projetos = consulta
            .query_map([], projeto_da_linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projetos)
    }

    pub fn atualizar_projeto(&self, id: &str, dados: &DadosProjeto) -> Resultado<Projeto> {
        let alteradas = self.conexao.execute(
            &format!(
                "UPDATE projetos
                 SET nome = ?2, system_prompt = ?3, modelo_padrao = ?4, atualizado_em = {AGORA}
                 WHERE id = ?1"
            ),
            params![id, dados.nome, dados.system_prompt, dados.modelo_padrao],
        )?;
        if alteradas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        self.obter_projeto(id)
    }

    /// Quantas conversas, mensagens e memórias serão apagadas junto com o projeto.
    pub fn resumo_exclusao_projeto(&self, id: &str) -> Resultado<ResumoExclusaoProjeto> {
        self.obter_projeto(id)?;
        let contar = |sql: &str| -> Resultado<u64> {
            Ok(contagem(
                self.conexao.query_row(sql, [id], |linha| linha.get(0))?,
            ))
        };
        Ok(ResumoExclusaoProjeto {
            conversas: contar("SELECT COUNT(*) FROM conversas WHERE projeto_id = ?1")?,
            mensagens: contar(
                "SELECT COUNT(*) FROM mensagens
                 WHERE conversa_id IN (SELECT id FROM conversas WHERE projeto_id = ?1)",
            )?,
            memorias: contar("SELECT COUNT(*) FROM memorias WHERE projeto_id = ?1")?,
        })
    }

    /// Exclui o projeto e, em cascata, as conversas, mensagens e memórias dele. As memórias
    /// globais não são tocadas. A confirmação com o usuário é responsabilidade da tela.
    pub fn excluir_projeto(&self, id: &str) -> Resultado<()> {
        let apagadas = self
            .conexao
            .execute("DELETE FROM projetos WHERE id = ?1", [id])?;
        if apagadas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        Ok(())
    }

    // ----- Conversas -----

    pub fn criar_conversa(&self, projeto_id: &str, dados: &DadosConversa) -> Resultado<Conversa> {
        let id = novo_id();
        self.conexao.execute(
            "INSERT INTO conversas
                 (id, projeto_id, titulo, modelo, temperature, num_ctx, keep_alive)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                projeto_id,
                dados.titulo,
                dados.modelo,
                dados.temperature,
                dados.num_ctx,
                dados.keep_alive
            ],
        )?;
        self.obter_conversa(&id)
    }

    pub fn obter_conversa(&self, id: &str) -> Resultado<Conversa> {
        Ok(self.conexao.query_row(
            "SELECT * FROM conversas WHERE id = ?1",
            [id],
            conversa_da_linha,
        )?)
    }

    pub fn listar_conversas_do_projeto(&self, projeto_id: &str) -> Resultado<Vec<Conversa>> {
        let mut consulta = self
            .conexao
            .prepare("SELECT * FROM conversas WHERE projeto_id = ?1 ORDER BY criado_em, id")?;
        let conversas = consulta
            .query_map([projeto_id], conversa_da_linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversas)
    }

    pub fn atualizar_conversa(&self, id: &str, dados: &DadosConversa) -> Resultado<Conversa> {
        let alteradas = self.conexao.execute(
            &format!(
                "UPDATE conversas
                 SET titulo = ?2, modelo = ?3, temperature = ?4, num_ctx = ?5,
                     keep_alive = ?6, atualizado_em = {AGORA}
                 WHERE id = ?1"
            ),
            params![
                id,
                dados.titulo,
                dados.modelo,
                dados.temperature,
                dados.num_ctx,
                dados.keep_alive
            ],
        )?;
        if alteradas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        self.obter_conversa(id)
    }

    /// Exclui a conversa e, em cascata, as mensagens dela.
    pub fn excluir_conversa(&self, id: &str) -> Resultado<()> {
        let apagadas = self
            .conexao
            .execute("DELETE FROM conversas WHERE id = ?1", [id])?;
        if apagadas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        Ok(())
    }

    // ----- Mensagens -----

    /// Acrescenta uma mensagem ao fim da conversa.
    pub fn adicionar_mensagem(
        &self,
        conversa_id: &str,
        papel: Papel,
        conteudo: &str,
        raciocinio: Option<&str>,
    ) -> Resultado<Mensagem> {
        let id = novo_id();
        self.conexao.execute(
            "INSERT INTO mensagens (id, conversa_id, posicao, papel, conteudo, raciocinio)
             VALUES (
                 ?1, ?2,
                 (SELECT COALESCE(MAX(posicao), 0) + 1 FROM mensagens WHERE conversa_id = ?2),
                 ?3, ?4, ?5
             )",
            params![id, conversa_id, papel.como_texto(), conteudo, raciocinio],
        )?;
        self.obter_mensagem(&id)
    }

    pub fn obter_mensagem(&self, id: &str) -> Resultado<Mensagem> {
        Ok(self.conexao.query_row(
            "SELECT * FROM mensagens WHERE id = ?1",
            [id],
            mensagem_da_linha,
        )?)
    }

    /// Mensagens da conversa, na ordem em que foram trocadas.
    pub fn listar_mensagens(&self, conversa_id: &str) -> Resultado<Vec<Mensagem>> {
        let mut consulta = self
            .conexao
            .prepare("SELECT * FROM mensagens WHERE conversa_id = ?1 ORDER BY posicao")?;
        let mensagens = consulta
            .query_map([conversa_id], mensagem_da_linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(mensagens)
    }

    /// Troca o conteúdo de uma mensagem, mantendo o ID e a posição. É o que a regeneração
    /// de uma resposta usa: a resposta antiga deixa de existir.
    pub fn substituir_mensagem(
        &self,
        id: &str,
        conteudo: &str,
        raciocinio: Option<&str>,
    ) -> Resultado<Mensagem> {
        let alteradas = self.conexao.execute(
            &format!(
                "UPDATE mensagens
                 SET conteudo = ?2, raciocinio = ?3, atualizado_em = {AGORA}
                 WHERE id = ?1"
            ),
            params![id, conteudo, raciocinio],
        )?;
        if alteradas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        self.obter_mensagem(id)
    }

    // ----- Memórias -----

    pub fn criar_memoria_global(&self, conteudo: &str) -> Resultado<Memoria> {
        let id = novo_id();
        self.conexao.execute(
            "INSERT INTO memorias (id, escopo, projeto_id, conteudo)
             VALUES (?1, 'global', NULL, ?2)",
            params![id, conteudo],
        )?;
        self.obter_memoria(&id)
    }

    pub fn criar_memoria_de_projeto(&self, projeto_id: &str, conteudo: &str) -> Resultado<Memoria> {
        let id = novo_id();
        self.conexao.execute(
            "INSERT INTO memorias (id, escopo, projeto_id, conteudo)
             VALUES (?1, 'projeto', ?2, ?3)",
            params![id, projeto_id, conteudo],
        )?;
        self.obter_memoria(&id)
    }

    pub fn obter_memoria(&self, id: &str) -> Resultado<Memoria> {
        Ok(self.conexao.query_row(
            "SELECT * FROM memorias WHERE id = ?1",
            [id],
            memoria_da_linha,
        )?)
    }

    pub fn listar_memorias_globais(&self) -> Resultado<Vec<Memoria>> {
        let mut consulta = self
            .conexao
            .prepare("SELECT * FROM memorias WHERE escopo = 'global' ORDER BY criado_em, id")?;
        let memorias = consulta
            .query_map([], memoria_da_linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(memorias)
    }

    pub fn listar_memorias_do_projeto(&self, projeto_id: &str) -> Resultado<Vec<Memoria>> {
        let mut consulta = self
            .conexao
            .prepare("SELECT * FROM memorias WHERE projeto_id = ?1 ORDER BY criado_em, id")?;
        let memorias = consulta
            .query_map([projeto_id], memoria_da_linha)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(memorias)
    }

    pub fn atualizar_memoria(&self, id: &str, conteudo: &str) -> Resultado<Memoria> {
        let alteradas = self.conexao.execute(
            &format!("UPDATE memorias SET conteudo = ?2, atualizado_em = {AGORA} WHERE id = ?1"),
            params![id, conteudo],
        )?;
        if alteradas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        self.obter_memoria(id)
    }

    pub fn excluir_memoria(&self, id: &str) -> Resultado<()> {
        let apagadas = self
            .conexao
            .execute("DELETE FROM memorias WHERE id = ?1", [id])?;
        if apagadas == 0 {
            return Err(ErroBanco::NaoEncontrado);
        }
        Ok(())
    }

    // ----- Registro de exclusões -----

    /// Tipo da entidade excluída (`projeto`, `conversa`, `mensagem` ou `memoria`) se o ID
    /// consta no registro de exclusões.
    pub fn tipo_da_exclusao(&self, id: &str) -> Resultado<Option<String>> {
        Ok(self
            .conexao
            .query_row("SELECT tipo FROM exclusoes WHERE id = ?1", [id], |linha| {
                linha.get(0)
            })
            .optional()?)
    }
}
