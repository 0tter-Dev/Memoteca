-- Esquema inicial do Memoteca (E04).
--
-- Regras:
-- * IDs são UUIDv7 gerados pelo app, guardados como texto.
-- * Datas em UTC, formato ISO 8601 com milissegundos (ex.: 2026-11-08T12:00:00.000Z).
-- * A exclusão é física. A tabela `exclusoes` guarda só o ID, o tipo e a data de cada
--   exclusão, sem nenhum conteúdo, para um sync futuro e para que um ID nunca volte.
-- * Este arquivo roda dentro de uma transação aberta pelo executor de migrações:
--   não use BEGIN, COMMIT nem PRAGMA aqui.

-- Configurações do app (chave e valor). A própria chave é o identificador estável.
CREATE TABLE configuracoes (
    chave TEXT PRIMARY KEY NOT NULL,
    valor TEXT NOT NULL,
    criado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    atualizado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE projetos (
    id TEXT PRIMARY KEY NOT NULL,
    nome TEXT NOT NULL,
    system_prompt TEXT NOT NULL DEFAULT '',
    modelo_padrao TEXT,
    criado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    atualizado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- Toda conversa pertence a exatamente um projeto. Não existe conversa avulsa.
CREATE TABLE conversas (
    id TEXT PRIMARY KEY NOT NULL,
    projeto_id TEXT NOT NULL REFERENCES projetos (id) ON DELETE CASCADE,
    titulo TEXT NOT NULL,
    modelo TEXT,
    temperature REAL,
    num_ctx INTEGER,
    keep_alive TEXT,
    criado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    atualizado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX conversas_por_projeto ON conversas (projeto_id);

-- O system prompt nunca é gravado como mensagem: ele é montado a cada envio.
CREATE TABLE mensagens (
    id TEXT PRIMARY KEY NOT NULL,
    conversa_id TEXT NOT NULL REFERENCES conversas (id) ON DELETE CASCADE,
    posicao INTEGER NOT NULL,
    papel TEXT NOT NULL CHECK (papel IN ('user', 'assistant')),
    conteudo TEXT NOT NULL,
    raciocinio TEXT,
    criado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    atualizado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (conversa_id, posicao)
) STRICT;

-- Memória global não pertence a projeto algum. Memória de projeto sempre aponta
-- para um projeto existente e some junto com ele.
CREATE TABLE memorias (
    id TEXT PRIMARY KEY NOT NULL,
    escopo TEXT NOT NULL CHECK (escopo IN ('global', 'projeto')),
    projeto_id TEXT REFERENCES projetos (id) ON DELETE CASCADE,
    conteudo TEXT NOT NULL,
    criado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    atualizado_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (
        (escopo = 'global' AND projeto_id IS NULL)
        OR (escopo = 'projeto' AND projeto_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX memorias_por_projeto ON memorias (projeto_id);

-- Registro mínimo de exclusões: só ID, tipo e data. Nunca guarda conteúdo.
CREATE TABLE exclusoes (
    id TEXT PRIMARY KEY NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('projeto', 'conversa', 'mensagem', 'memoria')),
    excluido_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- Cada exclusão, inclusive as feitas em cascata, deixa o registro mínimo.
CREATE TRIGGER projetos_registra_exclusao AFTER DELETE ON projetos
BEGIN
    INSERT INTO exclusoes (id, tipo) VALUES (OLD.id, 'projeto');
END;

CREATE TRIGGER conversas_registra_exclusao AFTER DELETE ON conversas
BEGIN
    INSERT INTO exclusoes (id, tipo) VALUES (OLD.id, 'conversa');
END;

CREATE TRIGGER mensagens_registra_exclusao AFTER DELETE ON mensagens
BEGIN
    INSERT INTO exclusoes (id, tipo) VALUES (OLD.id, 'mensagem');
END;

CREATE TRIGGER memorias_registra_exclusao AFTER DELETE ON memorias
BEGIN
    INSERT INTO exclusoes (id, tipo) VALUES (OLD.id, 'memoria');
END;

-- Nenhum ID excluído pode voltar a ser usado.
CREATE TRIGGER projetos_id_nao_reutilizado BEFORE INSERT ON projetos
WHEN EXISTS (SELECT 1 FROM exclusoes WHERE id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'id_reutilizado');
END;

CREATE TRIGGER conversas_id_nao_reutilizado BEFORE INSERT ON conversas
WHEN EXISTS (SELECT 1 FROM exclusoes WHERE id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'id_reutilizado');
END;

CREATE TRIGGER mensagens_id_nao_reutilizado BEFORE INSERT ON mensagens
WHEN EXISTS (SELECT 1 FROM exclusoes WHERE id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'id_reutilizado');
END;

CREATE TRIGGER memorias_id_nao_reutilizado BEFORE INSERT ON memorias
WHEN EXISTS (SELECT 1 FROM exclusoes WHERE id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'id_reutilizado');
END;
