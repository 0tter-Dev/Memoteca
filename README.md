# Memoteca

> **Aviso de independência:** este é um projeto independente. Ele **não é afiliado, patrocinado, endossado nem mantido** pelo Ollama ou por seus mantenedores, e **não tem relação direta** com o projeto ou com a marca Ollama. "Ollama" é marca de seus respectivos titulares e aparece aqui apenas para descrever com qual software este aplicativo funciona. O aplicativo **não redistribui o Ollama**: ele é obtido da fonte oficial e permanece como software separado, sob a própria licença.

Aplicativo desktop para Windows que organiza e dá memória às suas conversas com modelos de linguagem locais rodando no [Ollama](https://ollama.com): projetos, conversas e memórias persistentes, com instalação e configuração guiadas, sem precisar abrir um terminal.

## Quick Start

Ainda não há código executável neste repositório. Os pré-requisitos de desenvolvimento e os comandos de execução serão documentados aqui na etapa E01 do [plano](PLANO.md).

## Escopo atual

O que o MVP vai fazer (detalhes e critérios em [`PLANO.md`](PLANO.md)):

- Instalar, iniciar e encerrar o Ollama por você, e baixar modelos com progresso visível.
- Onboarding guiado, com catálogo curado de modelos e campo livre para qualquer modelo.
- Projetos com system prompt fixo e modelo padrão; conversas dentro dos projetos, com streaming.
- Parâmetros por conversa: modelo, `temperature`, `num_ctx` e `keep_alive`.
- Memória manual em dois níveis (projeto e global), com sugestão sob demanda aprovada item a item.
- Exibição de raciocínio (`thinking`) colapsada, regeneração por substituição e exportar/importar como backup manual.
- Persistência local e mensagens de erro amigáveis, com recuperação automática quando possível.

Fora do escopo por enquanto: resumo acumulado do projeto, sincronização entre máquinas, parâmetros avançados de inferência, Ollama em segundo plano, atualização automática do Ollama, múltiplas janelas, cliente MCP, outras plataformas além do Windows e interface em outros idiomas além de pt-BR (`i18n` para inglês e espanhol é só uma possibilidade futura). A lista completa está em [`PLANO.md`](PLANO.md).

## Status

Planejamento concluído; o código ainda não foi iniciado. O próximo passo é a etapa **E01** (scaffold do app), que depende de o mantenedor criar o repositório git local e o remoto no GitHub. O quadro de etapas e o status de cada uma estão em [`PLANO.md`](PLANO.md).

## Stack

- **Shell desktop:** Tauri 2, com backend em Rust (a UI roda no WebView2; é um app instalado, não um site).
- **Frontend:** React + TypeScript.
- **Persistência:** SQLite local.
- **Inferência:** Ollama, via API HTTP local (`localhost:11434`).
- **CI:** GitHub Actions.
- **Idioma:** pt-BR.

Decisões ainda abertas estão no Registro de decisões do [`PLANO.md`](PLANO.md#registro-de-decisões).

## Licença

MIT. O arquivo `LICENSE` será adicionado na etapa E01. O repositório é público no GitHub.

## Documentação

- [`PLANO.md`](PLANO.md): escopo, regras de negócio, etapas, critérios de aceite e decisões. É a fonte de verdade do projeto.
- [`AGENTS.md`](AGENTS.md): regras para agentes de IA que contribuem com o código.
