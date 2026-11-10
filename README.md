# Memoteca

> **Aviso de independência:** este é um projeto independente. Ele **não é afiliado, patrocinado, endossado nem mantido** pelo Ollama ou por seus mantenedores, e **não tem relação direta** com o projeto ou com a marca Ollama. "Ollama" é marca de seus respectivos titulares e aparece aqui apenas para descrever com qual software este aplicativo funciona. O aplicativo **não redistribui o Ollama**: ele é obtido da fonte oficial e permanece como software separado, sob a própria licença.

Aplicativo desktop para Windows que organiza e dá memória às suas conversas com modelos de linguagem locais rodando no [Ollama](https://ollama.com): projetos, conversas e memórias persistentes, com instalação e configuração guiadas, sem precisar abrir um terminal.

## Quick Start

O app ainda é só um esqueleto (janela única com o nome do projeto). Para desenvolver no Windows, instale:

- [Node.js](https://nodejs.org) e [pnpm](https://pnpm.io) (a versão usada está em `packageManager` no `package.json`);
- [Rust](https://rustup.rs) estável, com as [ferramentas de build C++ do Visual Studio](https://tauri.app/start/prerequisites/#windows);
- WebView2, que já vem no Windows 11.

```bash
pnpm install
pnpm tauri dev
```

Comandos úteis:

| Comando | O que faz |
| --- | --- |
| `pnpm check` | Roda todas as verificações: formato, lint, tipos, testes e build do frontend, e formato, lint e testes do backend Rust. É o mesmo comando que o CI executa |
| `pnpm test` | Testes do frontend (Vitest) |
| `pnpm rust:test` | Testes do backend (`cargo test`) |
| `pnpm format` | Aplica o Prettier |
| `pnpm tauri build --no-bundle` | Gera o executável em `src-tauri/target/release/memoteca.exe`, sem instalador |

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

Etapas **E01** (scaffold do app e ferramentas de qualidade), **E02** (verificações automáticas no GitHub), **E03** (contrato com a API do Ollama, telas e fluxos) e **E04** (persistência SQLite) e **E05** (cliente da API do Ollama) e **E06** (ciclo de vida do Ollama) e **E07** (instalação silenciosa do Ollama) e **E08** (onboarding e catálogo de modelos) e **E09** (projetos) e **E10** (conversas e chat) e **E11** (memória manual) concluídas: todo PR e todo push na `main` rodam o `pnpm check` num runner Windows (workflow em [`.github/workflows/ci.yml`](.github/workflows/ci.yml)). Antes de abrir um PR, rode `pnpm check` localmente: ele precisa terminar sem erros.

O app abre o banco local ao iniciar, aplica as migrações e mostra uma mensagem amigável se não conseguir.

O backend sabe consultar a versão, listar modelos, baixar modelos e conversar com streaming, com erros tipados. Os testes rodam contra um servidor Ollama falso.

Ao abrir, o app detecta o Ollama, reutiliza o que já está rodando (e nunca o encerra) ou inicia o instalado, e mostra a versão ou uma mensagem amigável. O Ollama que o próprio app iniciou é encerrado ao fechar, inclusive se o app for finalizado à força.

Sem Ollama instalado, o app oferece instalá-lo: baixa o instalador oficial (cerca de 1,6 GB) com progresso, confere o SHA-256 e a assinatura digital da Ollama Inc., instala sem pedir administrador e desfaz tudo se algo falhar.

Na primeira vez, o app guia o usuário por boas-vindas, Ollama, escolha de um modelo (catálogo curado ou qualquer nome) e download com progresso, verificando o espaço em disco antes e permitindo continuar um download interrompido.

O app tem a lateral com os projetos, e dá para criar, editar e excluir projetos (com confirmação), cada um com suas instruções (system prompt) e um modelo padrão.

Dentro de um projeto dá para criar conversas e conversar com o modelo, com a resposta chegando em streaming, raciocínio recolhido, markdown, cancelar e gerar de novo. Cada conversa guarda o próprio modelo e parâmetros, e o histórico fica salvo.

Há uma tela de memórias, com as do projeto e as globais (criar, editar e remover). A cada mensagem enviada, o app junta as instruções do projeto, as memórias globais e as do projeto numa só mensagem `system`. Editar ou remover uma memória só vale para as próximas mensagens. Se o contexto da conversa chega perto do `num_ctx`, a tela avisa. 

Etapa **E12** (sugestão de memórias) em revisão: o botão "Sugerir memórias" da conversa faz uma chamada extra ao modelo da própria conversa e abre uma lista de sugestões. Cada uma só vira memória quando você a aprova, escolhendo se vale para o projeto ou para todos. Rejeitar ou fechar a lista não guarda nada, e o histórico da conversa não muda. O quadro de etapas e o status de cada uma estão em [`PLANO.md`](PLANO.md).

## Stack

- **Shell desktop:** Tauri 2, com backend em Rust (a UI roda no WebView2; é um app instalado, não um site).
- **Frontend:** React + TypeScript, com Vite, pnpm, Vitest, ESLint e Prettier.
- **Interface:** Tailwind CSS 4, Radix UI e Zustand (em uso desde a E09). `react-markdown` com `remark-gfm` e `rehype-highlight` para as respostas (links e imagens aparecem só como texto).
- **Persistência:** SQLite local, via `rusqlite` (SQLite embutido), com migrações próprias e IDs UUIDv7. O banco fica em `%APPDATA%io.github.0tter-dev.memotecamemoteca.db`, com cópias de segurança em `backups` quando uma migração é aplicada.
- **Inferência:** Ollama 0.30 ou mais recente, via API HTTP local (`127.0.0.1:11434`), com cliente em Rust (`reqwest`, `tokio`).
- **Testes ponta a ponta (decidido, entram na E15):** Playwright conectado ao WebView2.
- **CI:** GitHub Actions (runner Windows, permissões mínimas, actions fixadas por commit).
- **Idioma:** pt-BR.

Decisões ainda abertas estão no Registro de decisões do [`PLANO.md`](PLANO.md#registro-de-decisões).

## Licença

MIT, conforme o arquivo [`LICENSE`](LICENSE). O repositório é público no GitHub.

## Documentação

- [`PLANO.md`](PLANO.md): escopo, regras de negócio, etapas, critérios de aceite e decisões. É a fonte de verdade do projeto.
- [`AGENTS.md`](AGENTS.md): regras para agentes de IA que contribuem com o código.
