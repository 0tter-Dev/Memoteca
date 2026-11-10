-- E10: uma resposta do assistente pode ser guardada pela metade (o usuário cancelou ou a
-- conexão caiu no meio). Esta coluna diz se a mensagem está `completa` ou `interrompida`.
-- As mensagens que já existiam são todas completas.
ALTER TABLE mensagens ADD COLUMN estado TEXT NOT NULL DEFAULT 'completa'
    CHECK (estado IN ('completa', 'interrompida'));
