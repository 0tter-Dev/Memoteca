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

Etapa **E01** (scaffold do app e ferramentas de qualidade) concluída. Etapa **E02** (verificações automáticas no GitHub) em revisão: todo PR e todo push na `main` rodam o `pnpm check` num runner Windows (workflow em [`.github/workflows/ci.yml`](.github/workflows/ci.yml)). Antes de abrir um PR, rode `pnpm check` localmente: ele precisa terminar sem erros. O quadro de etapas e o status de cada uma estão em [`PLANO.md`](PLANO.md).

## Stack

- **Shell desktop:** Tauri 2, com backend em Rust (a UI roda no WebView2; é um app instalado, não um site).
- **Frontend:** React + TypeScript, com Vite, pnpm, Vitest, ESLint e Prettier.
- **Persistência:** SQLite local.
- **Inferência:** Ollama, via API HTTP local (`localhost:11434`).
- **CI:** GitHub Actions (runner Windows, permissões mínimas, actions fixadas por commit).
- **Idioma:** pt-BR.

Decisões ainda abertas estão no Registro de decisões do [`PLANO.md`](PLANO.md#registro-de-decisões).

## Licença

MIT, conforme o arquivo [`LICENSE`](LICENSE). O repositório é público no GitHub.

## Documentação

- [`PLANO.md`](PLANO.md): escopo, regras de negócio, etapas, critérios de aceite e decisões. É a fonte de verdade do projeto.
- [`AGENTS.md`](AGENTS.md): regras para agentes de IA que contribuem com o código.
