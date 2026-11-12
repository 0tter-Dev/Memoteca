# Plano do Projeto — Memoteca

Este arquivo é a **fonte de verdade de escopo, regras de negócio e ordem de trabalho** do projeto, ao lado do [`README.md`](README.md) e do [`AGENTS.md`](AGENTS.md). Ele consolida o antigo `Project_Concept.md` e o `PROJECT-BRIEF.md`, que podem ser descartados depois que você o revisar.

## Como usar este plano

- O plano é executado **uma etapa por vez, sob comando direto do usuário** (ex.: "execute a E04"). Nenhum agente inicia uma etapa por conta própria.
- Cada etapa cabe em **um PR revisável**. O agente implementa, valida, atualiza a documentação, faz o commit (Conventional Commits) e abre o PR. **O merge é sempre do usuário.**
- A ordem abaixo segue a dependência técnica entre as etapas e não é uma priorização de negócio. O usuário pode reordenar ou dividir etapas a qualquer momento.
- Em cada etapa, os **Critérios de aceite** são o que torna a etapa verificável. Cada um é objetivo, e os marcados com **(falha)** exercitam o caminho de erro. Todo critério precisa de pelo menos um teste automatizado, ou de um item nomeado no checklist manual da E15 quando não houver como automatizar.
- Itens **a definir** não são preenchidos por suposição. O agente propõe uma opção ao usuário, espera a decisão e a registra no [Registro de decisões](#registro-de-decisões).
- Ao concluir uma etapa, o mesmo PR atualiza a coluna **Status** do quadro abaixo, o `README.md` (Quick Start, Escopo atual, Stack e Status) quando o que o projeto roda ou usa mudou, e o Registro de decisões.

## Quadro de etapas

| Etapa | Título | Depende de | Status |
| --- | --- | --- | --- |
| E01 | Scaffold do app e ferramentas de qualidade | repositório git criado pelo mantenedor | concluída |
| E02 | Verificações automáticas no GitHub (CI) | E01 | concluída |
| E03 | Contrato com a API do Ollama, telas e fluxos | E01 | concluída |
| E04 | Persistência SQLite e modelo de dados | E01 | concluída |
| E05 | Cliente da API do Ollama (HTTP e streaming) | E03 | concluída |
| E06 | Ciclo de vida do Ollama | E05 | concluída |
| E07 | Instalação silenciosa do Ollama | E06 | concluída |
| E08 | Onboarding e catálogo de modelos | E07 | concluída |
| E09 | Projetos | E03, E04 | concluída |
| E10 | Conversas e chat com streaming | E05, E09 | concluída |
| E11 | Memória manual (projeto e global) e injeção | E10 | concluída |
| E12 | Sugestão de memórias sob demanda | E11 | concluída |
| E13 | Exportar e importar | E04, E11 | concluída |
| E14 | Robustez transversal: erros amigáveis e recuperação | E07, E08, E10, E13 | concluída |
| E15 | Testes funcionais ponta a ponta do MVP | E14 | em revisão |
| E16 | Empacotamento e release | E15 | pendente |

Valores de **Status**: `pendente`, `em andamento` (só após comando do usuário), `em revisão` (PR aberto) e `concluída` (PR mergeado).

---

## Referência do produto

### Independência em relação ao Ollama

Este projeto é **independente** e **não tem relação direta** com o Ollama enquanto projeto e marca. Não é afiliado, patrocinado, endossado nem mantido por ele.

- "Ollama" é marca de seus respectivos titulares. O nome só pode aparecer para descrever compatibilidade (ex.: "funciona com o Ollama"), nunca como se o app fosse um produto do Ollama.
- O app **não redistribui o Ollama**: o instalador oficial é obtido da fonte oficial e o Ollama continua sendo software separado, sob a própria licença.
- O nome do projeto é **Memoteca**, escolhido por não conter a marca nem induzir à ideia de produto oficial do Ollama. O nome `OllamaUI` foi apenas provisório e não deve aparecer em código, pacotes, instalador nem documentação.
- O aviso de independência aparece no `README.md`, na tela "Sobre" do app e no instalador. O app não usa logotipo, ícone nem identidade visual do Ollama.

### Visão

Aplicativo desktop para Windows que funciona como camada de interação, organização e memória sobre o **Ollama** (CLI/API em `localhost:11434`). O Ollama permanece como motor de inferência; o projeto **não reimplementa inferência**.

A promessa central é que qualquer usuário, inclusive quem nunca abriu um terminal e não tem Ollama nem modelos instalados, consiga instalar, configurar e usar LLMs locais por uma aplicação guiada.

O app oficial do Ollama não tem base de conhecimento persistente, organização em projetos, memória manual, múltiplos usuários nem cliente MCP. O diferencial deste projeto é a **camada de organização e memória**.

### Plataforma e distribuição

- Plataforma primária: Windows. App desktop instalado (`.exe`), construído com Tauri 2. A UI roda no WebView2, e não há servidor nem navegador.
- Instalador único, sem exigir terminal em nenhum momento do fluxo.
- **Janela única** e **uma conversa ativa por vez**, com `single-instance lock`.
- Interface 100% em **pt-BR**. `i18n` para inglês e espanhol é só uma possibilidade futura, sem compromisso de escopo.

### Relação com o Ollama

- Detectar instalação existente antes de qualquer download.
- Instalar silenciosamente pelo instalador oficial quando não houver instalação.
- Iniciar e encerrar o `ollama serve`. Por padrão, **encerrar ao fechar o app** (sem processos órfãos).
- **Ollama já em execução** (por exemplo, o app oficial iniciado com o Windows): reutilizar a instância que responde na porta e encerrar apenas o processo que o próprio app iniciou. Se outro programa ocupar a `11434`, mostrar erro amigável.
- Verificar a versão mínima no startup, com aviso amigável se estiver antiga.
- Baixar modelos via `POST /api/pull`, com progresso visível na UI.

### Organização: projetos e conversas

- Navegação: sidebar com projetos e, dentro deles, as conversas. **Não existem conversas avulsas fora de projetos.**
- **Projeto** é a unidade de contexto compartilhado. Cada um tem:
  - **system prompt fixo**, herdado por todas as conversas;
  - **lista de memórias do projeto**, injetadas em toda conversa do projeto;
  - **modelo padrão**, herdado pelas novas conversas. É configurável, e se não houver configuração vale o modelo da primeira conversa criada.
- **Conversa** pertence a exatamente um projeto e tem:
  - modelo selecionado (herdado do projeto, sobrescrevível);
  - parâmetros de inferência `temperature`, `num_ctx` e `keep_alive`;
  - título truncado da primeira mensagem do usuário, editável manualmente;
  - histórico de mensagens persistido.

### Memória

- Dois níveis, ambos manuais: **projeto** (todas as conversas do projeto) e **global** (todas as conversas de todos os projetos, ex.: "Possuo Windows 11").
- System prompt e memórias são **injetados a cada mensagem enviada**. Mensagens já geradas nunca mudam, alterações afetam só gerações futuras, e a exclusão de memória é silenciosa (sem indicador de "contexto mudou").
- Criação por tela dedicada (criar, editar e remover) ou por **sugestão sob demanda**: um botão dispara uma chamada extra ao modelo, e o usuário aprova ou rejeita cada sugestão. **Nada vira memória sem aprovação explícita.**

### Raciocínio e regeneração

- O campo `thinking` fica **colapsado por padrão**, com indicador discreto expansível. Se o modelo não emitir `thinking`, a seção não aparece. Não há toggle por conversa.
- Regenerar **substitui** a resposta antiga. Não há histórico de versões.

### Exportação e persistência

- Export/import leve (JSON ou Markdown) como backup manual e saída de emergência se o banco corromper.
- Persistência local em SQLite. As entidades têm IDs estáveis e timestamps, para viabilizar sync futuro. **Não há sync entre máquinas no MVP.**

### Tratamento de erros

Toda falha gera **mensagem amigável** e, sempre que possível, recuperação, rollback ou limpeza automática. Validações prévias antes de ações custosas:

- espaço em disco antes de baixar um modelo (alerta ou bloqueio);
- porta `11434` antes de iniciar o servidor;
- instalação existente do Ollama antes de instalar.

Recuperação automática: rollback de instalação parcial e limpeza de download incompleto.

### Fora do escopo do MVP

| Item | Motivo |
| --- | --- |
| Nível 3 de contexto (resumo acumulado das conversas do projeto) | Exige etapa de sumarização; complexidade desproporcional |
| Sincronização entre máquinas | Exige backend, autenticação e modelo pronto para merge |
| Painel de parâmetros avançados (`top_p`, `top_k`, `repeat_penalty`, `seed`, `stop`…) | O MVP expõe só o núcleo |
| Opção de manter o Ollama rodando em segundo plano | O padrão é encerrar ao fechar |
| Notificação e atualização automática do Ollama | O MVP só verifica a versão mínima |
| Múltiplas abas ou janelas | Janela única, uma conversa por vez |
| Toggle de `thinking` por conversa | Colapsado por padrão basta |
| Indicador de "contexto mudou" | Exclusão de memória é silenciosa |
| Título de conversa gerado por modelo | Truncar e editar manualmente resolve |
| Histórico de versões de respostas regeneradas | Substituição simples no MVP |
| Cliente MCP | Fora do escopo declarado |
| Plataformas além do Windows | Foco inicial |
| Interface em outros idiomas | Só pt-BR; `i18n` en/es é possibilidade futura |

### Definição de "MVP pronto"

O MVP está pronto quando um usuário Windows, **sem Ollama instalado e sem nenhum modelo baixado**, consegue:

1. Instalar o app, abri-lo e ser guiado pelo onboarding até ter pelo menos um modelo baixado, sem abrir terminal em momento algum.
2. Criar um projeto, definir seu system prompt e adicionar memórias manualmente.
3. Abrir uma conversa no projeto, escolher modelo e parâmetros (`temperature`, `num_ctx`, `keep_alive`) e conversar com streaming.
4. Receber sugestões de memória a partir da conversa (sob demanda) e aprovar ou rejeitar cada uma.
5. Ver as memórias do projeto injetadas automaticamente em novas conversas dele, e as globais em qualquer conversa de qualquer projeto.
6. Exportar e importar dados como backup manual.
7. Fechar e reabrir o app e encontrar tudo persistido.
8. Em qualquer falha previsível (disco cheio, porta ocupada, download interrompido), ver mensagem amigável e, quando possível, recuperação automática.

### Stack e decisões consolidadas

- **Decididas:** Tauri 2 com backend em Rust; React + TypeScript; SQLite local; Ollama via API HTTP local; instalador `.exe` único; pt-BR; nome Memoteca; licença MIT, com repositório público no GitHub; GitHub Actions como CI.
- **Pendentes:** ver o [Registro de decisões](#registro-de-decisões).

---

## Etapas

### E01 — Scaffold do app e ferramentas de qualidade

**Depende de:** repositório git local e remoto criados manualmente pelo mantenedor.

**Entrega:**

- Projeto Tauri 2 + React + TypeScript (Vite) que compila e abre no Windows, com nome do app, do pacote e do identificador Tauri baseados em "Memoteca".
- Janela única com `single-instance lock`.
- Qualidade de código: TypeScript em modo estrito, ESLint e Prettier no frontend, `rustfmt` e `clippy` no backend.
- Runners de teste: `cargo test` e um runner para o frontend (Vitest sugerido).
- Um **comando único local** que roda as mesmas verificações do CI da E02.
- `.gitignore` e `LICENSE` (MIT).
- `README.md` com Quick Start real.

**Critérios de aceite:**

- O comando de desenvolvimento abre a janela do app e o de build gera o executável.
- Abrir o app uma segunda vez não cria nova janela e traz a primeira para frente. **(falha)**
- O comando único de verificação termina com código diferente de zero quando há erro de formato, lint, tipo ou teste. Cada tipo é provado uma vez com uma mudança proposital descartada depois. **(falha)**
- `LICENSE` contém a licença MIT.

**Decisões a tomar:** gerenciador de pacotes (`npm`, `pnpm`…) e runner de teste do frontend.

**Documentação:** `README.md` (Quick Start, Stack, Status).

### E02 — Verificações automáticas no GitHub (CI)

**Depende de:** E01.

**Entrega:**

- Workflow do GitHub Actions executado em `pull_request` e em `push` para a branch principal, em runner Windows.
- O workflow roda formato, lint, tipos, testes Rust e frontend e build, com cache de dependências.
- Template de PR com checklist: etapa, critérios atendidos, testes, README atualizado e Registro de decisões atualizado.
- Lista de ações manuais do mantenedor no GitHub: proteger a branch principal exigindo os checks e o PR, e restringir quem faz merge.

**Critérios de aceite:**

- Um PR correto fica com todos os checks verdes.
- Um PR com falha proposital de lint, tipo ou teste fica com check vermelho. **(falha)**
- O workflow declara permissões mínimas (`contents: read`) e não depende de segredos. Como o repositório é público, ele não expõe segredos nem permissões de escrita a PRs vindos de forks. **(falha)**
- As versões das actions usadas estão fixadas.
- O workflow executa o mesmo conjunto do comando local da E01, sem verificações que só existam no CI.

**Decisões a tomar:** ferramenta de cobertura e se ela é exigida no CI. _Decidido: não exigir por enquanto, ver o Registro de decisões._

**Documentação:** `README.md` (Status e como rodar as verificações).

**Ações manuais do mantenedor no GitHub** (não dá para fazer por arquivo no repositório; faça depois do merge da E02, quando o check já existir):

1. **Proteger a `main`** em Settings → Rules → Rulesets (ou Branches): exigir PR antes do merge, exigir o check **Verificações (Windows)** e que a branch esteja atualizada, bloquear _force push_ e exclusão.
2. **Restringir quem faz merge:** exigir 1 aprovação e deixar só o seu usuário na lista de quem pode contornar a exigência de PR. Assim a conta do agente, que tem permissão Write para abrir branches, não consegue fazer merge.
3. **Reduzir o poder dos workflows** em Settings → Actions → General: permissão padrão do `GITHUB_TOKEN` somente leitura, não enviar segredos nem token de escrita a PRs de forks, e exigir aprovação para rodar workflows de quem contribui pela primeira vez.
4. **Exigir actions fixadas** na mesma tela, em "Allow actions and reusable workflows" (permitir só as selecionadas e exigir fixação por SHA de commit completo).

Ao atualizar uma action, troque o SHA e o comentário de versão no `ci.yml`, e o teste `todas_as_actions_estao_fixadas_por_commit` garante que nenhuma volte para uma tag.

### E03 — Contrato com a API do Ollama, telas e fluxos

**Depende de:** E01.

**Natureza:** etapa de design. A saída é escrita neste arquivo, nas seções "Contrato Ollama", "Telas e fluxos" e "Plano de testes de robustez" abaixo. Não há código de produto.

**Entrega:**

- **Contrato Ollama:** os endpoints usados em cada fluxo, com os campos relevantes. Provavelmente `/api/version`, `/api/tags`, `/api/pull`, `/api/chat` e `/api/show`, a confirmar na documentação oficial.
- **Telas e fluxos:** onboarding, sidebar e projeto, conversa, memórias, configurações, erros e a tela "Sobre", com o aviso de independência em relação ao Ollama.
- **Plano de testes de robustez:** a lista de cenários de falha previsíveis (instalação, download, porta ocupada, disco cheio, Ollama caindo durante o streaming) e como cada um é simulado.
- **Estratégia de testes:** como o Ollama é substituído por um servidor falso nos testes e qual ferramenta de E2E é usada (ver E15).

**Critérios de aceite:**

- Cada fluxo do MVP aparece em pelo menos uma tela descrita, e cada ação de uma tela aponta para um endpoint ou para uma operação local.
- Cada cenário de falha da seção 2.11 do conceito (disco cheio, porta ocupada, download interrompido, instalação parcial) tem um teste previsto no plano de robustez. **(falha)**
- As três seções acima existem neste arquivo e foram aprovadas pelo usuário antes de a E05 e a E09 começarem.

**Decisões a tomar:** versão mínima do Ollama, bibliotecas de UI (componentes, estado, markdown, realce de código) e ferramenta de E2E. _Decididas com o usuário, ver o Registro de decisões._

### E04 — Persistência SQLite e modelo de dados

**Depende de:** E01.

**Entrega:**

- Banco SQLite no diretório de dados do usuário.
- Entidades: configurações do app, projeto, conversa, mensagem e memória (escopo global ou de projeto).
- IDs estáveis e timestamps de criação e atualização em todas as entidades.
- Migrações versionadas.
- Camada de acesso a dados no backend Rust.

**Critérios de aceite:**

- Dados gravados são encontrados depois de fechar e reabrir o app.
- As migrações aplicam em banco vazio e em banco de uma versão anterior, e rodar de novo não altera nada (idempotência).
- Uma migração que falha deixa o banco como estava e mostra mensagem amigável. **(falha)**
- Uma memória global não pertence a nenhum projeto, e uma memória de projeto sempre aponta para um projeto existente.
- Nenhum ID é reaproveitado depois de uma exclusão.

**Decisões a tomar:** biblioteca SQLite (`rusqlite`, `sqlx`…), formato do ID (UUID, ULID…), exclusão física ou lógica (tombstones) pensando no sync futuro, o que acontece com conversas e memórias ao excluir um projeto, e backup do banco antes de migrar. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E04):**

- **Código:** módulo `banco` em `src-tauri/src/banco/` (`Banco`, migrações em `migracoes/*.sql`, `backup`, `erro`, `id`, `modelos`, `repositorio`). O banco é `memoteca.db` na pasta de dados do app (`%APPDATA%io.github.0tter-dev.memoteca`), e as cópias de segurança ficam em `backups` ao lado dele.
- **Tabelas:** `configuracoes`, `projetos`, `conversas`, `mensagens`, `memorias` e `exclusoes`. Todas usam `STRICT`. IDs são UUIDv7 em texto e as datas são ISO 8601 em UTC.
- **Mensagens** guardam só os papéis `user` e `assistant`, com o `thinking` em coluna própria. O system prompt nunca é gravado como mensagem. Modelo e parâmetros da conversa são opcionais, e os valores padrão são definidos na E10.
- **Interface:** na abertura, o app consulta o comando `estado_do_banco`. Se o banco não abrir, a tela mostra a mensagem amigável.

**Documentação:** `README.md` (Stack) e Registro de decisões.

### E05 — Cliente da API do Ollama (HTTP e streaming)

**Depende de:** E03.

**Entrega:**

- Módulo do backend que consulta a versão, lista modelos locais, baixa modelos com progresso e conversa com streaming, tudo conforme o contrato da E03.
- Suporte a `temperature`, `num_ctx` e `keep_alive`.
- Leitura separada de `content` e `thinking` no stream.
- Cancelamento de geração em andamento.
- Servidor Ollama falso para os testes.

**Critérios de aceite:**

- Um stream com chunks quebrados no meio de uma linha é reconstruído sem perda.
- Um stream sem `thinking` não gera erro nem seção vazia.
- Cancelar uma geração fecha a conexão e nenhum texto novo chega depois do cancelamento.
- Conexão recusada ou Ollama que cai no meio do stream produzem um erro tipado, e não pânico. **(falha)**
- Resposta com JSON inválido produz erro tipado, e o conteúdo parcial já recebido é preservado. **(falha)**
- Todos os testes rodam contra o servidor falso, sem Ollama real.

**Como ficou (E05):**

- **Código:** módulo `ollama` em `src-tauri/src/ollama/`. `ClienteOllama` tem `verificar` (`/api/version`), `listar_modelos` (`/api/tags`), `detalhes_do_modelo` (`/api/show`), `baixar_modelo` (`/api/pull`, com progresso) e `conversar` (`/api/chat`, com streaming). Os módulos de apoio são `erro` (`ErroOllama`, `FalhaChat`), `versao` (mínima `0.30.0`), `ndjson` (divisor de linhas em bytes) e `tipos`.
- **Conversa:** o raciocínio (`thinking`) e o conteúdo chegam separados e na ordem. `think` nunca é enviado, e o corpo leva só `temperature` e `num_ctx` em `options` e `keep_alive`. Qualquer falha devolve `FalhaChat`, com o texto parcial já recebido preservado. Cancelar devolve `Ok` com `MotivoFim::Cancelada` e fecha a conexão, e nenhum pedaço é entregue depois disso.
- **Servidor falso:** `src-tauri/tests/support/ollama_falso.rs`, em `tokio` puro (HTTP/1.1 escrito à mão, sem biblioteca de servidor), com injeção de falhas: linha partida, JSON inválido, queda no meio, silêncio, resposta que não é HTTP, conexão aceita e fechada. Ele detecta quando o cliente fecha a conexão. A E15 reaproveita o mesmo arquivo.
- **Achado no Windows:** a recusa de conexão a uma porta fechada leva cerca de 2 s, mesmo em `127.0.0.1`. Por isso o limite de conexão padrão é 3 s, e o estado "Ollama fora do ar" demora esse tempo para ser detectado (relevante para a E06).
- **Ainda sem comandos do Tauri:** o módulo é só do backend. Os comandos e canais que o ligam à interface entram junto das telas (E06, E08 e E10).
### E06 — Ciclo de vida do Ollama

**Depende de:** E05.

**Entrega:**

- Detecção de instalação existente.
- Verificação da porta `11434`.
- Iniciar e encerrar o `ollama serve`.
- Reutilização de um Ollama já em execução.
- Verificação da versão mínima no startup.

**Regras:**

- Detectar a instalação antes de qualquer download.
- Se já houver um Ollama respondendo na porta, reutilizá-lo e **nunca encerrá-lo**. Só o processo iniciado pelo app é encerrado ao fechar.
- Se outro programa ocupar a porta, não iniciar nada e mostrar erro amigável.
- Sem processos órfãos: o processo iniciado pelo app termina junto com o app, inclusive em fechamento forçado.

**Critérios de aceite:**

- Com um Ollama externo respondendo na porta, o app o usa e, ao fechar, ele continua rodando.
- Com um programa qualquer ocupando a `11434` e sem Ollama, o app não inicia processo algum e mostra mensagem amigável. **(falha)**
- Com o Ollama iniciado pelo próprio app, fechar o app encerra o processo, inclusive quando o app é finalizado à força. **(falha)**
- Versão abaixo da mínima mostra aviso amigável e não bloqueia o uso de forma irrecuperável.
- Sem Ollama instalado, o estado "não instalado" encaminha para o onboarding.

**Decisões a tomar:** versão mínima exata (provavelmente ligada ao suporte ao campo `thinking`, a confirmar) e mecanismo de encerramento garantido (um Job Object do Windows é a sugestão). _Decididas: a versão mínima é a 0.30.0 (E03) e o encerramento garantido é por Job Object, ver o Registro de decisões._

**Como ficou (E06):**

- **Código:** em `src-tauri/src/ollama/`, `instalacao` (detecção por `PATH` e pelo local padrão), `processo` (`IniciadorDeProcesso` e `ProcessoFilho`, com a versão real e a falsa dos testes), `job` (o Job Object, único módulo com `unsafe`) e `ciclo` (`GerenciadorOllama`, `EstadoOllama` e `Origem`).
- **Fluxo de `preparar`:** consulta `/api/version`. Se responde um Ollama, reutiliza (`Origem::Externo`) e conta os modelos. Se responde outro programa, devolve `PortaOcupada` sem iniciar nada. Se ninguém responde, procura a instalação: sem ela, `NaoInstalado`; com ela, inicia `ollama serve`, espera responder (30 s) e passa a ser dono (`Origem::IniciadoPeloApp`). Falhas viram `Indisponivel` com `ErroOllama` tipado (`FalhaAoIniciar`, `TempoEsgotado`).
- **Encerramento:** só o processo que o app iniciou é encerrado, ao fechar o app e também ao descartar o gerenciador. Cada processo iniciado entra num Job Object que o Windows fecha se o app morrer, e isso encerra também os filhos (o executor do modelo).
- **Interface:** na abertura, a tela chama o comando `preparar_ollama` e mostra a versão, o aviso de versão antiga, a falta de modelos ou a mensagem amigável do problema. A tela de onboarding que o estado "não instalado" deve abrir é da E08, por ora só aparece a mensagem.
- **Verificado no executável real** com o Ollama 0.32.9 instalado na máquina: o app inicia o Ollama parado; fechar o app (normal ou à força) encerra o processo; um Ollama externo é reutilizado e continua rodando depois de fechar o app; com outro programa na `11434` nada é iniciado e a mensagem aparece.

### E07 — Instalação silenciosa do Ollama

**Depende de:** E06.

**Entrega:**

- Download do instalador oficial do Ollama.
- Validação do instalador antes de executá-lo.
- Execução silenciosa.
- Rollback de instalação parcial.
- Limpeza do arquivo baixado.
- Progresso na UI.

**Critérios de aceite:**

- Em uma máquina sem Ollama, a instalação termina e o app passa a detectá-lo, sem terminal.
- Uma falha no meio da instalação deixa o sistema sem resíduo do app e mostra mensagem amigável, com opção de tentar de novo. **(falha)**
- Sem internet, o erro é amigável e nada fica pela metade. **(falha)**
- Sem espaço em disco suficiente, a instalação é bloqueada antes de começar. **(falha)**
- O instalador baixado é removido ao fim, com sucesso ou falha.
- A execução do instalador passa por uma abstração que os testes substituem por uma versão falsa. A instalação real é verificada no checklist manual da E15.

**Decisões a tomar:** origem e parâmetros exatos do instalador silencioso, como o instalador é validado (assinatura digital, hash…) e como tratar o pedido de elevação de privilégio do Windows. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E07):**

- **Código:** módulo `instalador` em `src-tauri/src/instalador/`: `fonte` (release e download com SHA-256), `disco` (`EspacoEmDisco`), `assinatura` (`ValidadorDeAssinatura`), `executor` (`ExecutorDoInstalador`) e `erro`. `InstaladorDoOllama::instalar` orquestra o fluxo. As três abstrações, mais o detector de instalação da E06, são as que os testes substituem por versões falsas.
- **Fluxo:** recusa se já há Ollama instalado; consulta a release mais recente em `api.github.com/repos/ollama/ollama/releases/latest` (URL, tamanho e SHA-256 do `OllamaSetup.exe`); confere o espaço e **bloqueia antes de baixar**; baixa com progresso calculando o SHA-256; confere tamanho, hash e assinatura (publicador `Ollama Inc.`); executa o instalador; confere que o `ollama.exe` apareceu.
- **Espaço necessário:** tamanho do instalador (hoje 1,57 GB) mais 4 GB para a instalação (documentação do Ollama no Windows). Se a pasta de download e a de instalação estão na mesma unidade, os valores somam, pois o instalador só é apagado ao fim. Os modelos exigem espaço à parte (E08).
- **Rollback:** o instalador baixado é removido em qualquer desfecho. Uma instalação parcial é desfeita pelo desinstalador (`unins000.exe`) e, se a pasta ainda existir, ela é removida, **mas só se não existia antes da tentativa**. O app nunca apaga uma pasta que já era do usuário e nunca toca nos modelos. Se algo não puder ser removido (por exemplo, arquivo em uso), a mensagem avisa.
- **Interface:** no estado "não instalado", a tela mostra o botão "Instalar o Ollama", o progresso (fase e, no download, porcentagem e tamanho), "Cancelar" e, se falhar, a mensagem amigável com "Tentar de novo". Ao concluir, a tela detecta e inicia o Ollama. O onboarding completo é da E08.
- **Verificado com dados reais, sem instalar:** a consulta da release, o TLS do Windows e os redirecionamentos funcionam, a assinatura do `ollama.exe` instalado nesta máquina é aceita com o publicador `Ollama Inc.` e recusada com outro, e o espaço livre é lido corretamente. O download completo e a execução do instalador real **não** foram feitos aqui (a máquina já tem Ollama), e ficam no checklist manual da E15, em Windows limpo.

### E08 — Onboarding e catálogo de modelos

**Depende de:** E07.

**Entrega:**

- Fluxo guiado: detectar ou instalar o Ollama, escolher um modelo e baixá-lo, e entrar no app.
- **Catálogo curado** embutido no app (JSON, sem backend). Cada item tem nome amigável, tag real, tamanho, requisito estimado de RAM/VRAM e descrição curta.
- Campo livre para digitar qualquer modelo.
- Verificação de espaço em disco antes de baixar.
- Download com progresso e limpeza de download incompleto.

**Critérios de aceite:**

- Um usuário sem Ollama e sem modelos chega, só pela UI, a ter um modelo baixado e ao app principal.
- O catálogo é validado por um teste contra um esquema: todo item tem os campos obrigatórios e nenhuma tag duplicada. **(falha)**
- Com espaço insuficiente, o download é bloqueado ou alertado antes de começar, conforme a regra definida. **(falha)**
- Um download interrompido deixa o estado limpo e mostra mensagem amigável. **(falha)**
- Um modelo inexistente digitado no campo livre produz mensagem amigável, e não o erro cru da API. **(falha)**

**Decisões a tomar:** lista inicial de modelos do catálogo, margem de espaço que diferencia alerta de bloqueio, e se um download interrompido é limpo ou retomado. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E08):**

- **Código:** módulo `modelos` em `src-tauri/src/modelos/`: `catalogo` (carrega e valida o JSON embutido em `src-tauri/catalogo/modelos.json`), `nome` (validação do nome digitado), `espaco` (regra de espaço) e `servico` (`ServicoDeModelos`, o download guiado). Sem dependência nova.
- **Catálogo:** 6 modelos em 3 faixas, com tag real, nome amigável, descrição, tamanho exato em bytes (soma das camadas no registro do Ollama em 2026-11-08), memória estimada e se tem raciocínio. A validação exige todos os campos, recusa campos desconhecidos, tags inválidas ou repetidas (sem diferenciar maiúsculas), texto vazio e tamanho zero. Tags e tamanhos mudam com o tempo, então o catálogo é revisado a cada release do Memoteca.
- **Campo livre:** aceita `[host/][namespace/]modelo[:tag]` só com letras, números, `.`, `-` e `_`. Esquemas, espaços, portas, `..` e mais de 3 segmentos são recusados com mensagem amigável.
- **Espaço:** medido na unidade da pasta de modelos (`OLLAMA_MODELS`, ou `%USERPROFILE%.ollamamodels`). Para modelo do catálogo, o bloqueio acontece **antes de começar**, e a tela mostra o alerta e pede confirmação. Durante o download, o espaço que **falta baixar** é conferido a cada 1,5 s (isso cobre o modelo digitado à mão, cujo tamanho só aparece depois que o download começa, e o disco que encheu no meio). Se deixar de caber, o download é interrompido. Espaço ilegível não bloqueia.
- **Download:** `ServicoDeModelos::baixar` valida o nome, aplica a regra de espaço, baixa com progresso (as fases do Ollama viram `preparando`, `baixando`, `verificando`, `finalizando` e `concluido`) e confere que o modelo apareceu em `/api/tags` (com `:latest` quando a tag foi omitida). Antes de baixar, o comando garante que o Ollama está no ar, então "Continuar" também o reinicia se ele caiu.
- **Erros:** `ErroOllama::ModeloInexistente` cobre o `404` e a linha `pull model manifest: file does not exist` (confirmada na E06). Interrupção e tempo esgotado mostram "o que já foi baixado foi guardado" e o botão **Continuar o download**.
- **Correção na E06 (achada na E08):** com a pasta de modelos vazia, o Ollama 0.32.9 abre a porta e fica mais de 3 s sem responder direito, e o app o confundia com "outro programa" e o encerrava. Agora, enquanto o processo que o app iniciou está vivo, "porta ocupada" é tratada como "ainda subindo", e só vale se o processo sair ou se o prazo (30 s) acabar. Há um teste para isso.
- **Unidade de tamanho:** a tela mostra GB como 1024³ bytes, como o Windows, então o `llama3.2:3b` (2.019.393.189 bytes) aparece como 1,9 GB.
- **Interface:** o onboarding (T02) tem as boas-vindas (com o aviso de independência), o Ollama (verificar, instalar ou corrigir), o modelo (catálogo por faixa e campo livre), o download (progresso, cancelar, continuar) e o pronto. Quem já concluiu antes pula as boas-vindas, e o onboarding volta se faltar Ollama ou modelo. `onboarding_concluido` fica no banco.

### E09 — Projetos

**Depende de:** E03, E04.

**Entrega:**

- Criar, editar e excluir projetos.
- System prompt do projeto.
- Modelo padrão do projeto.
- Sidebar com a lista de projetos.

**Critérios de aceite:**

- O projeto criado, com seu system prompt, persiste depois de fechar e reabrir o app.
- O modelo padrão configurado é o que as novas conversas do projeto herdam.
- Sem modelo padrão configurado, vale o modelo da primeira conversa criada no projeto.
- Não existe caminho na UI para criar conversa fora de um projeto.
- Excluir um projeto segue a regra definida na E04 e pede confirmação antes de apagar. **(falha)**

**Como ficou (E09):**

- **Backend:** em `banco`, `criar_projeto` e `atualizar_projeto` validam os dados (nome obrigatório, sem espaços nas pontas e com até 100 caracteres; instruções com até 20.000 caracteres; modelo padrão com nome válido, e em branco vira "sem modelo") e devolvem mensagem amigável em pt-BR (`ErroBanco::Rejeitado`). A lista vem em **ordem alfabética**, sem diferenciar maiúsculas. Comandos do Tauri: `listar_projetos`, `criar_projeto`, `atualizar_projeto`, `resumo_exclusao_projeto`, `excluir_projeto` e `listar_modelos_instalados`.
- **Modelo herdado:** `modelo_herdado_pelo_projeto` aplica a regra: vale o **modelo padrão do projeto**; sem ele, o modelo da **primeira conversa criada** no projeto que tenha modelo (e não o da última); sem nenhum, a tela pede a escolha. `criar_conversa_herdando_modelo` cria a conversa já com esse modelo, e só existe dentro de um projeto. A E10 usa essas duas funções.
- **Interface:** a tela principal tem a lateral com os projetos (e o estado do Ollama) e a área central com o projeto aberto, o formulário de novo projeto ou o de edição. O botão "+" e o "Criar o primeiro projeto" abrem o formulário (nome, instruções, modelo padrão escolhido entre os instalados, com a opção "usar o modelo da primeira conversa"). **Não existe "nova conversa" em lugar nenhum fora de um projeto** (um teste confere).
- **Exclusão:** "Excluir projeto" abre uma confirmação (Radix `AlertDialog`) que mostra quantas conversas, mensagens e memórias serão apagadas junto, avisa que as memórias globais não são afetadas e que a ação não pode ser desfeita. Só dá para confirmar depois de o resumo carregar. Cancelar não apaga nada.
- **Bibliotecas de UI adotadas** (decididas na E03): Tailwind CSS 4 (`tailwindcss` e `@tailwindcss/vite`), Radix UI (`@radix-ui/react-alert-dialog`) e Zustand (estado dos projetos). `react-markdown` e `rehype-highlight` entram na E10, que mostra markdown.
### E10 — Conversas e chat com streaming

**Depende de:** E05, E09.

**Entrega:**

- Criar conversas dentro de um projeto.
- Escolha de modelo (herdado do projeto, sobrescrevível) e dos parâmetros `temperature`, `num_ctx` e `keep_alive`.
- Conversa com streaming e botão para cancelar a geração.
- Título truncado da primeira mensagem do usuário, editável.
- Histórico persistido.
- `thinking` colapsado.
- Regeneração por substituição.
- Montagem do contexto de cada envio (system prompt do projeto, mais memórias a partir da E11).

**Critérios de aceite:**

- O texto aparece em streaming, e o histórico continua depois de fechar e reabrir o app.
- O system prompt do projeto entra em **cada** mensagem enviada, e não só na primeira.
- Cada conversa guarda os próprios parâmetros e o próprio modelo, e mudar os de uma não altera as outras.
- Regenerar descarta a resposta antiga e deixa só a nova. **(falha)**
- Uma resposta sem `thinking` não mostra a seção, e uma com `thinking` mostra o bloco colapsado.
- Enviar com o Ollama fora do ar mostra mensagem amigável e **não perde** a mensagem do usuário. **(falha)**
- Só uma conversa fica ativa por vez.

**Decisões a tomar:** tamanho do truncamento do título, e comportamento de excluir conversa e editar mensagem, que o conceito não cobre. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E10):**

- **Backend:** módulo `conversas` (`titulo`, `contexto`, `geracao`, `controle`). `registrar_mensagem_do_usuario` grava a mensagem **antes** de qualquer chamada ao Ollama, então ela nunca se perde. `gerar` roda em dois modos: **responder** a última mensagem do usuário (a resposta vira mensagem nova) e **regenerar** (a resposta nova **substitui** a antiga, no mesmo ID e na mesma posição). `ControleDeGeracao` garante **uma geração por vez**. Comandos do Tauri: `listar_conversas`, `criar_conversa`, `listar_mensagens`, `atualizar_conversa`, `resumo_exclusao_conversa`, `excluir_conversa`, `enviar_mensagem`, `gerar_resposta_pendente` ("tentar de novo"), `regenerar_resposta`, `cancelar_geracao` e `detalhes_do_modelo`.
- **Contexto de cada envio:** uma única mensagem `system` com as instruções do projeto, **sempre a primeira e em todo envio** (e omitida se as instruções estão em branco), seguida do histórico só com `role` e `content`. O `thinking` de respostas anteriores nunca é reenviado. As memórias entram nessa mesma mensagem a partir da E11.
- **Streaming:** o raciocínio e o conteúdo chegam à tela separados, na ordem. O botão **Parar** cancela, fecha a conexão e guarda o que já chegou.
- **Resposta guardada pela metade:** migração 2 (`estado` da mensagem: `completa` ou `interrompida`). Se o usuário cancelar ou a conexão cair com texto já recebido, esse texto é guardado como `interrompida` (a tela mostra "Resposta interrompida." e oferece gerar de novo). Se a falha acontece ao **regenerar**, a resposta antiga é mantida. Cancelar sem nenhum texto não grava nada.
- **Parâmetros por conversa:** cada conversa guarda o próprio modelo, `temperature`, `num_ctx` e `keep_alive`. Uma conversa nova herda o modelo do projeto (regra da E09) e nasce com temperatura 0,7, contexto 8192 e 5 minutos na memória. O `num_ctx` enviado é limitado à janela máxima do modelo (`/api/show`). Limites validados: temperatura de 0 a 2, contexto de 512 a 4.194.304, e `keep_alive` `-1`, `0` ou número com `s`, `m` ou `h`.
- **Título:** a primeira mensagem dá o título (primeira linha, até 40 caracteres, cortando no fim de palavra e com "…"). Um título que o usuário editou nunca é sobrescrito.
- **Falhas:** Ollama fora do ar, modelo removido ou sem suporte a conversa, resposta vazia e JSON inválido viram mensagem amigável **dentro da conversa**, com "Tentar de novo". A mensagem do usuário continua lá. Antes de gerar, o comando garante que o Ollama está no ar (reinicia o que o app iniciou, se caiu).
- **Interface:** a lateral mostra as conversas do projeto aberto (da mais recente para a mais antiga) e o "+ Nova conversa", que só existe **dentro de um projeto**. A conversa tem título editável, "Modelo e parâmetros", a lista de mensagens (markdown com tabelas, listas, código realçado), o bloco "Raciocínio" **recolhido por padrão e só quando o modelo o emitiu**, "Gerar de novo" na última resposta, "Excluir conversa" com confirmação (diz quantas mensagens somem) e o campo de envio (Enter envia, Shift+Enter quebra a linha). Durante uma geração, a lateral fica travada: **só uma conversa ativa**, e ela não muda no meio de uma resposta.
- **Markdown seguro:** `react-markdown` com `remark-gfm` e `rehype-highlight`, sem HTML cru. Links e imagens aparecem como texto, sem nada clicável nem carregado.
- **Limitação conhecida:** durante o streaming, a resposta inteira é refeita em markdown a cada pedaço. Respostas muito longas (na validação real, uma de ~150 mil caracteres) deixam a tela lenta até terminar ou ser cancelada. Respostas normais (alguns milhares de caracteres) não sofrem com isso. Fica como ponto de atenção para a revisão.

### E11 — Memória manual (projeto e global) e injeção

**Depende de:** E10.

**Entrega:**

- Tela dedicada para criar, editar e remover memórias de projeto e globais.
- Injeção de system prompt e memórias a cada mensagem enviada.

**Critérios de aceite:**

- Uma memória global entra no contexto de conversas de qualquer projeto, e uma memória de projeto só no seu projeto.
- Editar ou excluir uma memória muda apenas as gerações seguintes, e as mensagens já geradas ficam idênticas. **(falha)**
- A exclusão de memória é silenciosa: a UI não exibe indicador de "contexto mudou".
- Uma memória removida não aparece no contexto da próxima mensagem. **(falha)**
- Memórias persistem depois de fechar e reabrir o app.

**Decisões a tomar:** ordem de montagem do contexto (system prompt, global, projeto) e o que fazer quando o contexto montado se aproxima do `num_ctx`. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E11):**

- **Banco:** nenhuma migração nova. A tabela `memorias` e as operações de criar, listar, editar e excluir já existiam desde a E04. O que entrou foi a validação do texto: sem espaços nas pontas, não pode ficar em branco e tem no máximo 2.000 caracteres (`MEMORIA_MAXIMA`).
- **Contexto de cada envio:** `montar_contexto` junta numa única mensagem `system`, nesta ordem, as **instruções do projeto**, as **memórias globais** (bloco "Memórias globais (valem para qualquer projeto):") e as **memórias do projeto** (bloco "Memórias deste projeto:"), cada memória como item de lista. Bloco sem conteúdo é omitido, e sem nenhum bloco não há mensagem `system`. As memórias são lidas do banco **a cada envio**, na mesma leitura que pega o projeto e o histórico. Por isso editar ou excluir uma memória só muda as gerações seguintes, e nenhuma mensagem já gravada é tocada.
- **Contexto perto do limite:** o app estima o tamanho do contexto enviado (1 token a cada 3 caracteres, mais 4 por mensagem, sem tokenizador) e, a partir de **80% do `num_ctx`**, a resposta traz um aviso. A tela mostra "O contexto desta conversa está quase cheio…" (ou "passou do limite…") e sugere aumentar o contexto em "Modelo e parâmetros". O app **não corta nem bloqueia nada**: o Ollama é quem descarta as mensagens mais antigas, e as instruções e memórias seguem em todo envio. O aviso some no envio seguinte.
- **Comandos do Tauri:** `listar_memorias_globais`, `listar_memorias_do_projeto`, `criar_memoria`, `atualizar_memoria` e `excluir_memoria`.
- **Interface:** tela de memórias (T06) com as abas "Deste projeto" e "Globais", aberta pelo "◇ Memória global" no fim da lateral ou pelo "Gerenciar memórias" no projeto. Cada memória tem "Editar" e "Remover", e há o campo "Nova memória" com contador de caracteres. **Remover é silencioso**: apaga na hora, sem diálogo de confirmação e sem nenhum aviso de "contexto mudou". Sem projeto aberto só existe a aba global. A lateral e o botão ficam desabilitados enquanto uma resposta é gerada, como no resto da E10.

### E12 — Sugestão de memórias sob demanda

**Depende de:** E11.

**Entrega:**

- Botão "sugerir memórias desta conversa", que faz uma chamada extra ao modelo.
- Lista de sugestões, com aprovar ou rejeitar cada uma.

**Critérios de aceite:**

- Aprovar uma sugestão cria exatamente uma memória, com o escopo escolhido.
- Rejeitar uma sugestão, ou fechar a tela sem decidir, não cria memória alguma. **(falha)**
- Nenhuma memória é criada sem aprovação explícita.
- Resposta do modelo malformada ou vazia mostra mensagem amigável e não cria memória. **(falha)**
- A chamada extra não altera nem acrescenta mensagens ao histórico da conversa.

**Decisões a tomar:** qual modelo gera as sugestões e se o escopo (projeto ou global) é escolhido na aprovação. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E12):**

- **Nada é criado pelo backend.** O comando `sugerir_memorias` só devolve uma lista de textos: não grava memória nem mensagem. Cada sugestão vira memória apenas quando o usuário a aprova na tela, pelo mesmo comando `criar_memoria` das memórias manuais (uma sugestão aprovada, uma memória). Rejeitar, ou fechar o diálogo sem decidir, não passa pelo backend e não cria nada. A chamada extra não toca no histórico da conversa nem na própria conversa.
- **A chamada:** `POST /api/chat` com `stream: false`, `format` (JSON Schema `{"sugestoes":[{"texto":"..."}]}`, no máximo 5 itens), `think: false` e `temperature` 0,2. Usa o **modelo da própria conversa**, o `num_ctx` e o `keep_alive` dela (o `num_ctx` limitado à janela do modelo, como no chat). O pedido tem uma instrução em português e a conversa em texto corrido (só o trecho mais recente, até 12.000 caracteres, sem o raciocínio). As memórias que já existem (globais e do projeto) vão na instrução com "não repita".
- **Validação da resposta (R31):** o conteúdo precisa ser um JSON no esquema. Resposta vazia, que não é JSON ou fora do esquema vira `RespostaInvalida`, com a mensagem amigável "O modelo não devolveu as sugestões num formato que o Memoteca entenda. Nenhuma memória foi criada…" e botão "Tentar de novo". **Uma lista vazia não é erro**: a tela diz que o modelo não achou nada que valha a pena guardar. Textos em branco, maiores que o limite de uma memória (2.000 caracteres) e repetidos (entre si ou iguais a uma memória existente, sem diferenciar maiúsculas) são descartados.
- **Cancelar:** a chamada usa a mesma "vaga" da geração de respostas (`ControleDeGeracao`), então não roda junto com uma resposta. Fechar o diálogo enquanto o modelo responde cancela a chamada e fecha a conexão com o Ollama, e uma resposta que chegue tarde é descartada. O tempo máximo é o `ociosidade_chat` (120 s), que aqui vale como tempo total porque nenhum byte chega antes de a resposta ficar pronta.
- **Interface (T07):** botão "Sugerir memórias" no cabeçalho da conversa (desabilitado sem mensagens ou durante uma resposta). Abre um diálogo (`role="dialog"`, Esc fecha) com "Analisando a conversa…" e "Cancelar", depois a lista. Cada sugestão tem a escolha "Deste projeto" (padrão) ou "Global", e os botões "Aprovar" e "Rejeitar". Se a aprovação falha, a sugestão continua na lista com a mensagem. Enquanto o diálogo está aberto, o envio de mensagens fica travado.
- **Sem migração nova e sem dependência nova.**
- **Limitação conhecida:** a conferência de repetidas é por texto igual. Na validação real, com um modelo de 1 bilhão de parâmetros, uma memória já guardada voltou numa segunda rodada reescrita com outras palavras ("Marina mora no Recife." no lugar de "O usuário mora no Recife."), e o modelo ignorou o "não repita". O usuário vê a sugestão e rejeita. Modelos maiores tendem a seguir melhor a instrução.

### E13 — Exportar e importar

**Depende de:** E04, E11.

**Entrega:**

- Exportação manual dos dados para arquivo (JSON ou Markdown).
- Importação desse arquivo.

**Critérios de aceite:**

- Exportar e depois importar em um banco vazio reproduz projetos, conversas, mensagens e memórias idênticos, com os mesmos IDs.
- Um arquivo inválido, truncado ou de versão desconhecida é recusado com mensagem amigável e **não altera** nenhum dado. **(falha)**
- A importação é atômica: se falhar no meio, nada fica importado pela metade. **(falha)**
- O arquivo exportado informa a versão do formato.
- Importar sobre dados existentes segue a política definida, sem duplicar itens com o mesmo ID.

**Decisões a tomar:** formato (JSON ou Markdown, ou os dois) e política de importação (mesclar ou substituir). _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E13):**

- **Formato:** um único arquivo **JSON** (formato `memoteca-backup`, **versão 1**), com `formato`, `versao`, `exportadoEm`, `app` e as listas `projetos`, `conversas`, `mensagens` (com o raciocínio e o estado `completa` ou `interrompida`) e `memorias`, com os mesmos IDs e datas do banco. As configurações do app (como "onboarding concluído") **não entram**: são da máquina. O arquivo não é criptografado, e a tela avisa.
- **Exportar:** a janela "Salvar como" do Windows (aberta pelo Rust) sugere `memoteca-backup-AAAA-MM-DD.json`. O banco é lido numa transação só (as tabelas são do mesmo instante) e o arquivo é gravado de forma **atômica**: escreve num arquivo temporário ao lado, confere que chegou ao disco e só então troca pelo destino. Se qualquer passo falhar (sem permissão, disco cheio, pasta inexistente), o temporário é apagado, **nenhum arquivo parcial fica** e um backup antigo no mesmo lugar não é tocado (R30).
- **Verificar antes de gravar (R28):** ler o arquivo não grava nada. A ordem é: é JSON? tem a marca `memoteca-backup`? a versão é a 1? (uma versão diferente é "versão desconhecida", com a mensagem "atualize o Memoteca", e não "arquivo inválido"); depois a estrutura e os dados: IDs UUID válidos e únicos, referências que fecham dentro do arquivo (conversa → projeto, mensagem → conversa, memória de projeto → projeto), posições de mensagem únicas, datas no formato do app e os mesmos limites de quando o usuário cria (nome, título, parâmetros, memória). Arquivo maior que 200 MB é recusado. Qualquer recusa diz "Nada foi importado nem alterado".
- **Importar mescla, sem apagar nem sobrescrever:** um item cujo ID já existe é **mantido como está**; uma conversa que já existe fica inteira, com as mensagens que tem. Um item que o usuário **excluiu aqui** é ignorado (o banco não aceita de volta um ID excluído), e o que depende dele (as conversas e memórias de um projeto excluído) também. O resto é adicionado. Num banco vazio, restaura tudo com os mesmos IDs. Importar duas vezes dá o mesmo resultado de uma.
- **Atômica (R29):** a importação roda numa transação e, se qualquer inserção falhar no meio, tudo é desfeito. O teste injeta a falha com um gatilho que recusa uma mensagem depois de projetos e conversas já terem entrado.
- **Confirmação:** escolher o arquivo só **analisa** (a mesma lógica da importação, desfeita no fim) e mostra o que vai acontecer: o que será adicionado, o que já existe e fica como está, e o que foi excluído aqui e não volta. O backup analisado fica guardado no backend, então o que é confirmado é exatamente o que foi analisado. Sem nada novo, a tela só deixa fechar.
- **Comandos do Tauri:** `exportar_backup`, `analisar_backup`, `importar_backup_analisado` e `descartar_backup_analisado`. O caminho do arquivo nunca passa pelo JavaScript: a janela é aberta pelo Rust.
- **Interface (T08):** botão "⚙ Configurações" no fim da lateral, com a seção "Dados" ("Exportar backup…" e "Importar backup…"). Por enquanto a tela só tem essa seção. As de Ollama e Modelos da T08 continuam fora.
- **Dependência nova:** `tauri-plugin-dialog` (só o crate Rust), que traz `tauri-plugin-fs` e `rfd` como dependências dele. Sem pacote npm e sem permissão nova para o JavaScript.

### E14 — Robustez transversal: erros amigáveis e recuperação

**Depende de:** E07, E08, E10, E13.

**Entrega:**

- Catálogo central de mensagens de erro em pt-BR.
- Revisão de todos os fluxos contra o plano de testes de robustez da E03.
- Testes de injeção de falha para cada cenário.

**Critérios de aceite:**

- Nenhum erro técnico cru (stack trace, texto de API, código interno) chega à UI nos cenários cobertos. **(falha)**
- Cada cenário do plano de robustez tem um teste que o provoca e verifica a mensagem e a recuperação ou limpeza.
- Cobertos: disco cheio, porta ocupada, download interrompido, instalação parcial, Ollama caindo durante o streaming, banco indisponível e arquivo de importação inválido. **(falha)**
- Depois de cada falha coberta, o app continua utilizável sem precisar ser reiniciado, exceto quando isso for documentado como limitação aprovada pelo usuário.

**Decisões a tomar:** local e retenção do log, o que fazer quando o banco não abre e a forma do catálogo de mensagens. _Decididas com o usuário, ver o Registro de decisões._

**Como ficou (E14):**

- **Log local:** o módulo `log` grava os detalhes técnicos em `logs/memoteca.log`, na pasta de dados (`%APPDATA%\io.github.0tter-dev.memoteca`), no lugar dos `eprintln!`. Cada arquivo vai até **1 MB** e ficam os **3 mais recentes** (`memoteca.log`, `memoteca.1.log`, `memoteca.2.log`). Cada linha é `<data UTC> ERRO <detalhe>`, numa linha só (quebras viram espaço, no máximo 600 caracteres). **Nunca entra o que o usuário escreve nem o que o modelo responde**: o detalhe de um backup recusado e de uma linha de chat inválida fala só do tipo e do lugar do erro, porque o texto de erro do `serde` cita o valor. Se o arquivo não puder ser gravado, o app segue sem log. Sem dependência nova.
- **Catálogo de mensagens (`catalogo.rs`):** os textos continuam com cada tipo de erro (`mensagem_amigavel`), e o catálogo **lista todas** as mensagens que o usuário pode ver (83 hoje: Ollama, banco, instalação, modelos, geração, sugestões, backup, exportação e estado do Ollama). Um teste confere cada uma: em português, frase completa que termina com pontuação, **diz o que fazer** (salvo as só informativas), e **sem texto técnico** (SQL, HTTP, JSON, caminhos, endereços, códigos, nomes de tipos nem o detalhe que é só do log). Um `match` exaustivo impede esquecer uma variante nova. Outro teste aplica as mesmas regras às mensagens escritas direto nos comandos do app (`lib.rs`). A conferência achou e corrigiu mensagens sem ação ("Não há mensagem para responder.", "Não encontramos este item…") e uma com quebra de linha e espaços dentro do texto (modelo inexistente).
- **Segunda barreira no frontend (`erros.ts`):** um único `mensagemDoErro` (que substituiu as cópias espalhadas pelas telas e stores) só deixa passar texto amigável. Erro do Tauri (argumento inválido, comando inexistente), exceção do JavaScript, objeto, vazio ou texto com cara de erro técnico vira a mensagem padrão. Os 83 textos do catálogo passam pelo filtro sem serem barrados (conferido).
- **Cobertura do plano de robustez:** o teste `todo_cenario_do_plano_de_robustez_tem_ao_menos_um_teste` lê as linhas `Rnn` do `PLANO.md` e exige um teste que cite cada cenário (`fn rNN_` em Rust, ou `Rnn` no nome de um teste do frontend), menos os cenários cujo teste é só da E15 (o R32). Faltava o **R33**, que ganhou testes.
- **R26 (dados que não abrem):** o `EstadoBanco` passou a poder ser trocado, então `tentar_abrir_banco` reabre o banco **sem reiniciar o app**. A tela vira um diálogo ("Não foi possível abrir seus dados") com a mensagem amigável, "Tentar de novo" e "Abrir a pasta dos dados" (`abrir_pasta_de_dados`, no Explorador de Arquivos), onde ficam o banco, as cópias de segurança e o log. O app **não apaga nem move** o arquivo. Se der certo, a tela lê tudo de novo e segue.
- **R33 (fechar o app no meio):** o banco fica consistente, e o teste fecha uma geração no meio, reabre o banco do arquivo (`integrity_check` ok), confere que só a mensagem do usuário ficou (sem resposta pela metade nem duplicada) e que dá para responder de novo. Na tela, uma conversa cuja última mensagem é do usuário e sem erro avisa "Esta mensagem ficou sem resposta…" e oferece **"Gerar resposta"**.
- **Verificação do Ollama que falha:** a tela com a mensagem ganhou "Tentar de novo", então o app não fica preso até reiniciar.
- **Limitação aprovada:** um Ollama que cai **durante o uso** não mostra uma faixa permanente; o próximo envio o reinicia sozinho (se foi o app que o iniciou) ou mostra a mensagem na conversa, com "Tentar de novo". A faixa persistente da T10 para esse caso fica para depois.

### E15 — Testes funcionais ponta a ponta do MVP

**Depende de:** E14.

**Entrega:**

- Testes funcionais automatizados que cobrem os 8 critérios de "MVP pronto", contra um servidor Ollama falso, rodando no CI.
- Checklist manual em Windows limpo (VM), para o que não se automatiza: instalação real do Ollama, download real de modelo e instalação do app sem terminal.

**Critérios de aceite:**

- Cada um dos 8 critérios do MVP aponta para pelo menos um teste automatizado ou um item nomeado do checklist manual.
- Os testes automatizados rodam no workflow da E02, e uma regressão proposital em um dos critérios derruba o check. **(falha)**
- O checklist manual foi executado uma vez em Windows limpo, com o resultado registrado no PR.
- O teste do fluxo "fechar e reabrir" realmente reinicia o app, e não só recarrega a tela.

**Decisões a tomar:** ferramenta de E2E para o Tauri no Windows (decidida na E03: Playwright por CDP, confirmada pelo piloto desta etapa). _Onde o E2E roda, o pacote do Playwright e onde fica o checklist manual foram decididos com o usuário, ver o Registro de decisões._

**Como ficou (E15):**

- **Piloto e ferramenta:** o teste-piloto (abrir o app, interagir, encerrar o processo e abrir outro) passou de primeira, então **fica o Playwright por CDP**, e o `tauri-driver` não foi necessário. O piloto foi absorvido pelos testes do critério 7. Dependência nova (avisada e aprovada): `@playwright/test`, só de desenvolvimento, sem baixar navegador (o Playwright só se conecta ao WebView2 do app).
- **Como roda:** `pnpm e2e` compila o servidor falso e o app e roda os testes. O `pnpm e2e:build` faz `cargo build --example ollama_falso` e `tauri build --debug --no-bundle --features e2e` (executável em `src-tauri/target/debug`, sem instalador). No CI, o passo `pnpm e2e` vem **depois do `pnpm check`**, no mesmo job (o check obrigatório), sem `continue-on-error`. Fica fora do `pnpm check` porque o E2E **se recusa a rodar onde há um Ollama de verdade** (a porta 11434), como a máquina de quem desenvolve. Um teste (`tests/ci.rs`) confere o passo e a ordem.
- **Recurso de compilação `e2e` (só no build de teste):** troca a pasta de dados do app (`MEMOTECA_E2E_DADOS`), as janelas "Exportar" e "Importar" por uma pasta de escolhas do teste (`MEMOTECA_E2E_ESCOLHAS`), o instalador do Ollama por um falso (consulta e baixa do servidor falso, aceita a assinatura e "instala" copiando o Ollama falso para a pasta padrão, sem executar o arquivo baixado) o espaço em disco por um valor que o teste muda com o app aberto (`espaco-livre.txt`) e a criação da janela, que no build de teste recebe a porta de depuração (CDP) por argumento (`src-tauri/e2e.tauri.conf.json` deixa a lista de janelas vazia, e `e2e::criar_janela` a cria). **Por quê:** o `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, que o plano previa, funcionou na máquina local mas foi ignorado pelo WebView2 no runner do GitHub (o app abria e a porta nunca aparecia). **Nunca entra no build de release:** não é recurso padrão, só o script `e2e:build` o liga, e testes em `tests/ci.rs` conferem isso.
- **Isolamento:** cada teste cria pastas temporárias (dados, escolhas, `LOCALAPPDATA`, modelos, perfil do WebView2, `TEMP`) e roda o app com um `PATH` só de pastas do sistema, então o único Ollama que existe é o falso, e os dados e o Ollama do usuário não são tocados. Antes de tudo, o E2E confere que nada responde na porta 11434.
- **Servidor falso executável:** `src-tauri/examples/ollama_falso.rs` reaproveita o `tests/support/ollama_falso.rs` (ganhou respostas dinâmicas e binárias) e responde como um Ollama que funciona. O teste escolhe cenários em tempo de execução (`normal`, `chat_corta`, `pull_interrompido`) e consulta os pedidos recebidos pelas rotas `/__e2e/`. Copiado como `ollama.exe` pelo instalador falso, ele é o Ollama que o app inicia e encerra, o que exercita o ciclo de vida de verdade (R03 e R04).
- **Fechar e reabrir é de verdade:** o teste encerra o processo do app pedindo à janela que feche (como o usuário) e abre **outro processo**, conferindo que o `pid` mudou, antes de olhar os dados.

| Critério de "MVP pronto" | Teste automatizado (`e2e/`) | Itens do checklist manual |
| --- | --- | --- |
| 1. Onboarding sem terminal até ter um modelo | `mvp-1-onboarding`: instala (simulado), inicia o Ollama, baixa um modelo e chega ao app; R03 e R04 (Ollama externo continua, o do app é encerrado) | M01, M02, M04, M09 |
| 2. Projeto, system prompt e memórias manuais | `mvp-2-5-projeto-conversa-memorias` (critério 2) | |
| 3. Conversa com modelo, parâmetros e streaming | `mvp-2-5-projeto-conversa-memorias` (critério 3: o pedido ao Ollama leva modelo, `temperature`, `num_ctx` e `keep_alive`) | M03 |
| 4. Sugestões sob demanda | `mvp-2-5-projeto-conversa-memorias` (critério 4: fechar sem decidir não guarda nada; só a aprovada vira memória) | M03 |
| 5. Memórias injetadas (projeto e global) | `mvp-2-5-projeto-conversa-memorias` (critério 5: confere a mensagem `system` enviada ao Ollama, a ordem e o escopo) | |
| 6. Exportar e importar | `mvp-6-7-backup-e-persistencia` (exporta de um app e importa em outro, sem duplicar; R28) | M06 |
| 7. Persistência ao fechar e reabrir | `mvp-6-7-backup-e-persistencia` (critério 7, com outro processo) | |
| 8. Falhas previsíveis com mensagem amigável | `mvp-8-falhas`: R02, R07, R09, R11, R16, R26 e R32. Cada teste confere que a tela só mostra texto amigável (sem termos técnicos) e que o app segue utilizável | M07 |

- **Regressão proposital derruba o check:** ao tirar as memórias do projeto da mensagem `system`, o teste do critério 5 falhou e o `pnpm e2e` saiu com código 1 (o passo do CI não tolera falha). A alteração foi desfeita.
- **R32 (segunda instância):** minimiza a janela, abre o executável outra vez e confere que a segunda execução termina sozinha, que continua um só processo e que a janela voltou. O cenário deixou de ser exceção no teste de cobertura de `tests/robustez.rs`, que agora também lê os nomes dos testes do `e2e/` (os que cobrem um `Rnn` o citam no título).
- **Não automatizado:** instalação e download reais, as janelas nativas do Windows, a elevação do Windows, Windows limpo e a instalação do app sem terminal. Vão para o checklist abaixo.

**Observação (fora do escopo, não corrigida):** ao mudar dois parâmetros da conversa em sequência quase instantânea (como faz um teste automático), o segundo pode sobrescrever o primeiro, porque cada gravação parte da conversa lida antes de a anterior terminar. Na mão não costuma acontecer, e os testes esperam cada valor aparecer antes do seguinte. Fica anotado para uma etapa de correções.

#### Checklist manual (E15)

Para executar **uma vez** em Windows limpo (VM, sem Ollama, sem modelo e sem ferramentas de desenvolvimento), antes do release, com o resultado registrado no PR. Não use a máquina de desenvolvimento: ela já tem Ollama.

| ID | Item | Como fazer | Resultado esperado | Critério |
| --- | --- | --- | --- | --- |
| M01 | Instalar o Ollama pelo app | Abrir o app, seguir o onboarding e clicar em "Instalar o Ollama" | Download de uns 1,6 GB com progresso, instalação sem pedir administrador, e o app mostra o Ollama em execução. **Anotar:** se o instalador silencioso deixa o Ollama rodando e se o `PATH` do usuário foi atualizado (item a validar nos princípios do contrato Ollama) | 1 |
| M02 | Baixar um modelo de verdade | No onboarding, baixar um modelo pequeno (por exemplo `llama3.2:1b`) | Progresso real, e o modelo aparece como instalado, sem abrir terminal | 1 |
| M03 | Conversar com o modelo real | Criar um projeto e uma conversa, enviar mensagens, parar uma resposta e pedir sugestões de memória | Streaming fluido, "Parar" funciona, e as sugestões vêm no formato esperado | 3, 4 |
| M04 | Fechar o app e o Ollama | Fechar o app pela janela. Repetir com o Ollama já aberto pelo ícone da bandeja | O Ollama iniciado pelo app encerra junto. O que já estava aberto continua | 1, 7 |
| M05 | Versão mínima do Ollama | Instalar à mão o Ollama 0.30.0 e abrir o app | Sem aviso de versão antiga, e conversa e sugestões funcionam. Resolve a decisão pendente sobre a versão mínima | 3 |
| M06 | Janelas nativas de exportar e importar | Em "Configurações", exportar e importar com as janelas do Windows | A janela abre na frente do app, com o filtro "Backup do Memoteca", e cancelar não altera nada | 6 |
| M07 | "Abrir a pasta dos dados" | Com os dados indisponíveis (arquivo que não é banco na pasta do app), clicar no botão | O Explorador de Arquivos abre na pasta do Memoteca | 8 |
| M08 | Pedido de elevação recusado | Se o instalador do Ollama pedir administrador, recusar | Mensagem amigável com "Tentar de novo" | 8 |
| M09 | Instalar o app sem terminal | Instalar o Memoteca com o instalador e abrir pelo atalho | **Depende do instalador da E16.** Até lá, abrir o `memoteca.exe` do `pnpm tauri build --no-bundle` e conferir só que abre e leva ao onboarding | 1 |

**Registro da execução** (preencher no PR): data, versão do Windows, ID do item, resultado (passou, falhou ou não se aplica) e observações. _Ainda não executado._

### E16 — Empacotamento e release

**Depende de:** E15.

**Entrega:**

- Instalador único `.exe` gerado pelo empacotador do Tauri.
- Workflow de release disparado por tag, que gera o instalador como artefato.
- Versionamento (série `0.x` até o MVP; a `1.0.0` marca o MVP pronto).
- `README.md` com instruções finais de instalação.
- Aviso de independência em relação ao Ollama na tela "Sobre" do app e no instalador.

**Critérios de aceite:**

- O aviso de independência aparece na tela "Sobre", no instalador e no `README.md`, e nenhum texto, ícone ou logotipo sugere que o app é um produto oficial do Ollama. **(falha)**
- O nome usado no instalador, na janela, nos metadados do executável e na release é "Memoteca", e "OllamaUI" não aparece em nenhum deles. **(falha)**

- O workflow de release gera o instalador a partir de uma tag e o anexa à release.
- Em Windows limpo, o instalador instala o app, que abre e leva ao onboarding, sem terminal.
- A desinstalação remove o app e trata os dados do usuário conforme a política definida.
- Uma tag com formato inválido não gera release. **(falha)**

**Decisões a tomar:** formato do instalador (NSIS ou MSI), assinatura de código (sem ela o Windows pode alertar o usuário), e se a desinstalação mantém ou apaga os dados do usuário.

---

## Registro de decisões

### Confirmadas

| Decisão | Escolha | Data |
| --- | --- | --- |
| Nível de documentação | Mínimo (README, AGENTS e este plano) | 2026-11-08 |
| Quem desenvolve | Solo, com agentes de IA autônomos que abrem os PRs; merge sempre do usuário | 2026-11-08 |
| Shell desktop | Tauri 2 com backend em Rust, foco em app desktop Windows | 2026-11-08 |
| Frontend | React + TypeScript | 2026-11-08 |
| Persistência | SQLite local | 2026-11-08 |
| Biblioteca SQLite e migrações | `rusqlite` com o SQLite embutido (`bundled`) e um executor de migrações próprio: arquivos `.sql` numerados, versão em `PRAGMA user_version` e todas as migrações pendentes numa única transação (tudo ou nada). Dependências novas: `rusqlite` e `uuid` | 2026-11-08 |
| Formato dos IDs | UUIDv7 em texto, gerado pelo app (`uuid` com a feature `v7`). Ordenado por data de criação e nunca reaproveitado | 2026-11-08 |
| Exclusão e cascata | Exclusão **física**, com registro mínimo na tabela `exclusoes` (só ID, tipo e data, sem conteúdo), gravado por gatilhos, inclusive nas exclusões em cascata. Um ID registrado não pode voltar a ser inserido. Excluir um projeto apaga em cascata as conversas, mensagens e memórias dele, e a tela de confirmação (E09) mostra as quantidades. Memórias globais não são tocadas | 2026-11-08 |
| Backup antes de migrar | Só quando há migração pendente em um banco que já existe: cópia por `VACUUM INTO` na pasta `backups`, mantendo as 3 mais recentes. Banco novo não gera cópia. Só arquivos com o nome `copia-v<versão>-<instante>.db` são apagados na limpeza | 2026-11-08 |
| Idioma | Interface e documentação em pt-BR; `i18n` en/es só como possibilidade futura | 2026-11-08 |
| Licença e visibilidade | MIT; repositório público no GitHub | 2026-11-08 |
| Nome do projeto | **Memoteca** ("memória" + "-teca"). Escolhido após triagem de conflitos: nenhum produto, empresa ou marca de IA com esse nome foi encontrado, só repositórios de alunos de cursos | 2026-11-08 |
| Ollama já em execução | Reutilizar a instância; encerrar só o processo iniciado pelo app | 2026-11-08 |
| Repositório e CI | Git local e remoto criados manualmente pelo mantenedor; checks e workflows no GitHub desenvolvidos nas etapas E02 e E15 | 2026-11-08 |
| Gerenciador de pacotes e runner de teste do frontend | pnpm e Vitest | 2026-11-08 |
| Identificador do app (Tauri) | `io.github.0tter-dev.memoteca`. Mudá-lo depois altera a pasta de dados do app no Windows, então só deve mudar antes do primeiro release | 2026-11-08 |
| Segurança do webview | Content-Security-Policy restritiva desde a E01 (`default-src 'self'`); novas origens só entram com aprovação | 2026-11-08 |
| Titular da licença MIT | "Memoteca contributors", no `LICENSE` e no `Cargo.toml`. Pode ser trocado por nome próprio a qualquer momento | 2026-11-08 |
| Relação com o Ollama | Projeto independente, sem afiliação nem relação direta com o Ollama (projeto e marca); aviso explícito na documentação, no app e no instalador | 2026-11-08 |
| Versão mínima do Ollama | **0.30.0**. O usuário pediu uma versão intermediária: recente o bastante para um app novo, sem ser a última para não gerar avisos sem necessidade. A 0.30.0 saiu em 2026-05-13 e a versão atual era a 0.40.15. O campo `thinking` do `/api/chat` entrou na 0.9.0, então o mínimo o cobre com folga. Revisar quando a E06 testar contra um Ollama real | 2026-11-08 |
| Bibliotecas de UI | Tailwind CSS 4 (estilo), Radix UI (componentes acessíveis), Zustand (estado de UI), `react-markdown` com `remark-gfm` (markdown, sem HTML cru) e `rehype-highlight` (realce de código). Entram como dependências só no PR que as usar pela primeira vez (E09 em diante). Dados de domínio ficam no backend Rust, e não em uma biblioteca de cache no frontend | 2026-11-08 |
| Ferramenta de E2E | Playwright conectado ao WebView2 por CDP. A E15 começa com um teste-piloto, e a alternativa, se falhar, é o `tauri-driver` | 2026-11-08 |
| Decisões de design do contrato Ollama (E03) | O Ollama só é acessado pelo backend Rust; endereço fixo `127.0.0.1:11434`; `think` não é enviado e o `thinking` do histórico não é reenviado; só `temperature` e `num_ctx` em `options`; sugestões de memória por `/api/chat` com `format` e sem stream. Valem como aprovadas com o merge da E03 | 2026-11-08 |
| Encerramento garantido do Ollama | Job Object do Windows por processo iniciado, com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, via `windows-sys` (dependência só do Windows, já presente na árvore do Tauri, com `unsafe` isolado em `job.rs`). Se o app for finalizado à força, o Windows fecha o job e encerra o Ollama e os filhos dele. Se não for possível criar o job, nada é iniciado | 2026-11-08 |
| Título automático da conversa | Primeira linha da primeira mensagem do usuário, até 40 caracteres, cortando no fim de palavra e acrescentando "…" se cortou. O título editado pelo usuário nunca é sobrescrito | 2026-11-09 |
| Excluir conversa e editar mensagem | Excluir conversa: sim, com confirmação que diz quantas mensagens somem (exclusão física em cascata, como no projeto). Editar mensagem: não, no MVP. Para outra resposta existe regenerar, e mensagens já geradas nunca mudam | 2026-11-09 |
| Links nas respostas do modelo | Aparecem como texto, não clicáveis, e imagens não carregam. Não usa o plugin `opener` nem permissão nova no Tauri. Se for preciso abrir links no futuro, é uma mudança de escopo a aprovar | 2026-11-09 |
| Parâmetros padrão da conversa | Temperatura 0,7, contexto (`num_ctx`) 8192 (limitado à janela do modelo) e `keep_alive` de 5 minutos, gravados em cada conversa nova | 2026-11-09 |
| Resposta guardada pela metade | Se o usuário cancela ou a conexão cai com texto já recebido, o texto é guardado como mensagem `interrompida`. Ao regenerar, uma falha mantém a resposta antiga. Migração 2 do banco | 2026-11-09 |
| Ordem do contexto montado (E11) | Uma única mensagem `system`, sempre a primeira: instruções do projeto, depois memórias globais, depois memórias do projeto (o mais específico por último, onde o modelo dá mais peso). Cada bloco tem título em português e as memórias vão em lista. Bloco vazio é omitido | 2026-11-09 |
| Contexto perto do `num_ctx` (E11) | Só avisar: estimativa por caracteres, aviso a partir de 80% do `num_ctx`, sem cortar histórico nem bloquear o envio. O Ollama já descarta as mensagens mais antigas sozinho, e instruções e memórias seguem em todo envio. Cortar ou bloquear ficaria para depois, se o aviso se mostrar pouco | 2026-11-09 |
| Tamanho e remoção de memórias (E11) | Máximo de 2.000 caracteres por memória (são fatos curtos que entram em todo envio). Remover apaga na hora, sem confirmação, porque o plano pede remoção silenciosa e a memória é um texto curto fácil de recriar | 2026-11-09 |
| Modelo das sugestões de memória (E12) | O da própria conversa (já carregado na memória, sem configuração nova). Se for pequeno demais, as sugestões podem ser fracas, e o usuário aprova ou rejeita cada uma | 2026-11-10 |
| Escopo das sugestões de memória (E12) | O usuário escolhe ao aprovar, com "Deste projeto" como padrão. O modelo não decide escopo, então nenhuma sugestão vira global por palpite dele | 2026-11-10 |
| Chamada das sugestões (E12) | `format` com JSON Schema e `think: false` (só nesta chamada; o chat continua sem `think`), temperatura 0,2, até 5 sugestões, conversa limitada a 12.000 caracteres recentes. Lista vazia do modelo não é erro. Comportamento de `format` e `think: false` conferido no Ollama 0.32.9 (ver o contrato) | 2026-11-10 |
| Formato do backup (E13) | Só JSON, com `formato` e `versao` (1). Cobre tudo, mantém os IDs e dá para importar de volta. Markdown legível fica para depois, se fizer falta | 2026-11-10 |
| Política de importação (E13) | Mesclar, mantendo o que já existe: o mesmo ID fica como está, sem duplicar nem sobrescrever; o que o usuário excluiu aqui é ignorado; nada é apagado. Num banco vazio restaura tudo. "Substituir tudo" não existe | 2026-11-10 |
| Escolha do arquivo do backup (E13) | Janela do Windows, com `tauri-plugin-dialog` (crate Rust; traz `tauri-plugin-fs` e `rfd`). O Rust abre a janela, então o caminho nunca passa pelo JavaScript e o plugin não ganha permissões para a interface. Avisado no PR da E13 | 2026-11-10 |
| Detalhes do backup (E13) | Fora do arquivo: as configurações do app e o registro de exclusões. Arquivo de até 200 MB. Gravação atômica por arquivo temporário e troca. Conversa que já existe é mantida inteira, sem mesclar mensagens | 2026-11-10 |
| Log local (E14) | Arquivo `logs\memoteca.log` na pasta de dados do app, com rodízio: até 1 MB por arquivo e 3 arquivos (uns 3 MB). Só detalhe técnico de erro, nunca o texto de conversas, memórias ou instruções. Sem dependência nova, no lugar dos `eprintln!` | 2026-11-11 |
| Banco que não abre (E14) | Diálogo com a mensagem amigável, "Tentar de novo" (reabre o banco sem reiniciar o app) e "Abrir a pasta dos dados". O app não apaga nem move o arquivo. "Começar com um banco novo" ficou de fora: mexe num arquivo do usuário, e o usuário pode fazer isso pela pasta | 2026-11-11 |
| Catálogo de mensagens (E14) | Os textos ficam com cada tipo de erro, e o módulo `catalogo` lista todas e as confere por teste (português, diz o que fazer, sem texto técnico). Não foi criado um módulo único com todos os textos, para não mexer em ~13 arquivos que funcionam | 2026-11-11 |
| Limites desta etapa (E14) | A faixa permanente para "Ollama caiu durante o uso" não entrou (o envio seguinte reinicia o Ollama do app). O R32 (segunda instância) segue como teste da E15 | 2026-11-11 |
| E2E no CI (E15) | O `pnpm e2e` roda como um passo do mesmo job, **depois** do `pnpm check`, e fica **fora** do `pnpm check`: o E2E se recusa a rodar onde há um Ollama de verdade na porta 11434, como a máquina de quem desenvolve. O `.github/workflows/ci.yml` deixa de rodar "só o `pnpm check`", e `tests/ci.rs` foi ajustado para aceitar exatamente os dois | 2026-11-11 |
| Pacote do Playwright (E15) | `@playwright/test` (dependência nova, só de desenvolvimento, sem baixar navegador: o Playwright só se conecta ao WebView2 do app por CDP). Traz o executor de testes e as esperas automáticas. A alternativa, `playwright-core`, deixaria executor e esperas por minha conta | 2026-11-11 |
| Ferramenta de E2E confirmada (E15) | O piloto passou de primeira (abrir, interagir, encerrar o processo e abrir outro), então fica **Playwright por CDP**, e o `tauri-driver` não é necessário | 2026-11-11 |
| Recurso `e2e` e servidor falso executável (E15) | Recurso de compilação `e2e` (inclusive a janela com a porta CDP por argumento, no lugar da variável de ambiente do WebView2, que o runner ignorou) (nunca padrão e nunca no release, conferido em `tests/ci.rs`) para trocar pasta de dados, janelas de arquivo, instalador e espaço em disco, como o plano de testes previa. Servidor falso como *example* do crate (`ollama_falso`), reaproveitando `tests/support/ollama_falso.rs`. Sem dependência nova em Rust | 2026-11-11 |
| Checklist manual (E15) | Fica numa seção do `PLANO.md` ("Checklist manual (E15)"), mantendo a regra de só README e PLANO como documentação. O usuário executa uma vez em Windows limpo (VM) **antes do merge**, e o resultado é registrado no PR. O item "instalar o app sem terminal" (M09) depende do instalador da E16 | 2026-11-11 |
| Dependências do chat (E10) | `react-markdown`, `remark-gfm` e `rehype-highlight`, todas já decididas na E03. O realce de código usa cores próprias (CSS), sem importar tema do `highlight.js`. Avisado no PR da E10 | 2026-11-09 |
| Regras de validação do projeto | Nome obrigatório, com até 100 caracteres (contados como caracteres, e não bytes); instruções opcionais, com até 20.000 caracteres; nomes de projeto repetidos são permitidos; lista em ordem alfabética sem diferenciar maiúsculas. Os limites são defesa contra textos absurdos e podem mudar | 2026-11-09 |
| Modelo herdado pelas conversas | Vale o modelo padrão do projeto; sem ele, o da primeira conversa criada no projeto que tenha modelo; sem nenhum, a tela pede a escolha. A conversa não grava o padrão: a regra é calculada a cada nova conversa | 2026-11-09 |
| Dependências de interface (E09) | `tailwindcss` e `@tailwindcss/vite` (dev), `@radix-ui/react-alert-dialog` e `zustand`, todas já decididas na E03. Avisado no PR da E09 | 2026-11-09 |
| Catálogo inicial de modelos | 6 modelos em 3 faixas: leves `llama3.2:3b` (2,0 GB) e `qwen3:4b` (2,5 GB); equilibrados `gemma3:4b` (3,3 GB), `qwen3:8b` (5,2 GB) e `llama3.1:8b` (4,9 GB); potente `gpt-oss:20b` (13,8 GB). Tamanhos conferidos no registro do Ollama em 2026-11-08, e a memória é estimativa | 2026-11-08 |
| Regra de espaço para modelos | Bloqueia se o espaço livre é menor que o tamanho do que falta baixar. Alerta, com confirmação do usuário, se depois do download sobrariam menos de 5 GB livres. A medição é na unidade da pasta de modelos do Ollama. Espaço ilegível não bloqueia | 2026-11-08 |
| Download de modelo interrompido | **Retomar.** O Ollama guarda o que já baixou e retoma do ponto onde parou, e o modelo só aparece na lista quando está completo. O Memoteca não apaga nada na pasta do Ollama. A mensagem amigável oferece "Continuar o download" | 2026-11-08 |
| Origem do instalador do Ollama | A **última versão oficial**: o app consulta a API de releases do GitHub do Ollama (a mesma release para a qual `ollama.com/download/OllamaSetup.exe` redireciona), que informa URL, tamanho e SHA-256, e baixa o `OllamaSetup.exe`. A URL de download precisa ser `https` e do host `github.com` | 2026-11-08 |
| Validação do instalador | SHA-256 conferido com o da release **e** assinatura digital válida com publicador `Ollama Inc.`, lida pelo PowerShell do Windows (`Get-AuthenticodeSignature`), sem dependência nova. Se qualquer uma falhar, o instalador não é executado e é removido | 2026-11-08 |
| TLS do cliente HTTP | TLS do Windows (`native-tls`, SChannel), com o proxy do sistema (`system-proxy`). O cliente do Ollama local continua sem TLS e sem proxy | 2026-11-08 |
| Execução do instalador e elevação | `OllamaSetup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-`, na pasta padrão do usuário, **sem elevar**: o Memoteca nunca pede administrador. Se o Windows exigir elevação (erro 740), a mensagem orienta a instalar pelo site oficial | 2026-11-08 |
| Dependências do instalador (E07) | `reqwest` com `native-tls` e `system-proxy` (traz `native-tls`, `schannel`, `windows-registry`), `sha2`, e as features `fs` e `io-util` do `tokio` e `Win32_Storage_FileSystem` do `windows-sys`. Avisado no PR da E07 | 2026-11-08 |
| Dependências do cliente Ollama (E05) | `reqwest` (sem TLS e sem proxy, só `stream`), `tokio`, `tokio-util` (cancelamento), `futures-util`, `serde` e `serde_json`. Todas já estavam na árvore de dependências do Tauri, e agora são declaradas diretamente. O servidor falso usa só `tokio` (dependência de desenvolvimento), sem `axum` ou `hyper` próprios. Avisado no PR da E05; vale como aprovada com o merge | 2026-11-08 |
| Cobertura de testes no CI | Não exigida por enquanto, sem ferramenta de cobertura nem dependência nova. O código ainda é mínimo e um percentual não diria nada. Reavaliar na E15, com o código do MVP pronto; se entrar, deve entrar também no `pnpm check`. **Reavaliada na E15:** continua não exigida, porque os critérios do MVP são cobertos por testes funcionais ponta a ponta, que um percentual de linhas não mediria, e ainda não há ferramenta de cobertura decidida. O usuário pode reabrir | 2026-11-08 |
| Significado de "build" no CI | O CI roda `pnpm check` e, desde a E15, também `pnpm e2e` (o app compilado em modo debug com o recurso `e2e`, sem LTO e sem instalador). O build é o do frontend (`tsc && vite build`, parte do `pnpm check`), e o Rust é compilado por `clippy` e `cargo test`. O executável (`tauri build --no-bundle`, com LTO) não roda no CI por custo, e o build do instalador entra na E16 | 2026-11-08 |

### Pendentes

| Decisão | Etapa que a resolve |
| --- | --- |
| Confirmar a versão mínima do Ollama (0.30.0) contra um Ollama real dessa versão. Na E06 só havia a 0.32.9 para testar | E15 (checklist manual) |
| Formato do instalador, assinatura de código, dados na desinstalação | E16 |
| Identidade visual (logo e paleta). Opcional, a discutir quando o usuário quiser, com a skill `project-identity`. Direção inicial: marca geométrica de "prateleiras e um ponto". Os ícones atuais são os padrões do Tauri e são provisórios | Antes de E16 |
| Consulta formal de registro de marca (INPI, USPTO, EUIPO) e do domínio, antes de divulgar amplamente. A triagem feita não substitui essa consulta | Mantenedor, antes de divulgar |

### Contrato Ollama

Escrito na E03. Fontes: documentação oficial do repositório do Ollama (`docs/api.md`, `docs/api/errors.mdx`, `docs/api/streaming.mdx`, `docs/capabilities/thinking.mdx`, `docs/faq.mdx` e `docs/windows.mdx`), lida em 2026-11-08, quando a versão mais recente era a 0.40.15. Itens marcados **(a validar)** não estão explícitos nessa documentação e devem ser conferidos contra um Ollama real na etapa indicada, antes de virarem teste.

#### Princípios

1. **Só o backend Rust fala com o Ollama.** A WebView não faz requisições a `11434`: a CSP (`connect-src`) não a permite e continua assim. A UI usa comandos e canais do Tauri.
2. **Endereço fixo `http://127.0.0.1:11434`.** O IP literal evita falhas de resolução de `localhost`. `OLLAMA_HOST` personalizado não é suportado no MVP.
3. **Formato.** Corpo JSON. Respostas em stream são NDJSON (um objeto JSON por linha, `application/x-ndjson`), e uma linha pode chegar partida entre dois pedaços de rede. Durações vêm em nanossegundos.
4. **Tolerância.** Campos desconhecidos são ignorados, e campos opcionais ausentes não são erro.
5. **Nome de modelo.** Formato `modelo:tag`, com `latest` quando a tag é omitida. O app guarda o nome exatamente como o `/api/tags` o devolve.
6. **Sem autenticação e só modelos locais.** Modelos em nuvem do Ollama exigem login e ficam fora do MVP. Se o `/api/tags` listar algum, o app não o oferece **(a validar na E05: como identificá-los)**.
7. **Mensagem de erro do Ollama nunca vai para a tela.** O texto cru (`error`) vai só para o log local; o usuário vê a mensagem amigável do tipo de erro correspondente.

#### Endpoints usados

| Endpoint | Para quê | Fluxos | Cliente |
| --- | --- | --- | --- |
| `GET /api/version` | Saber se há um Ollama respondendo, qual a versão e se é Ollama de fato | F1 | E05 |
| `GET /api/tags` | Listar modelos instalados | F1, F2, F3, F4 | E05 |
| `POST /api/show` | Capacidades e janela de contexto de um modelo | F4 | E05 |
| `POST /api/pull` | Baixar um modelo, com progresso | F2 | E05 |
| `POST /api/chat` | Conversar com streaming e gerar as sugestões de memória | F4, F6 | E05 |

Não usados no MVP: `/api/generate`, `/api/ps`, `/api/create`, `/api/copy`, `/api/delete`, `/api/push`, `/api/embed*` e os endpoints de compatibilidade com OpenAI e Anthropic.

#### `GET /api/version`

- Resposta: `{"version": "0.30.0"}`.
- **É Ollama** quando responde `200` com JSON que tem o campo `version` em texto. Qualquer outra coisa que responda na porta (HTML, JSON sem `version`, conexão aceita sem HTTP válido) é **outro programa**.
- Comparação de versão por número (`major.minor.patch`), ignorando sufixos como `-rc1`.
- **Versão mínima: `0.30.0`** (ver Registro de decisões). Abaixo disso: aviso amigável, sem bloqueio irrecuperável.

#### `GET /api/tags`

- Resposta: `{"models": [{"name", "model", "size", "modified_at", "digest", "details": {"family", "parameter_size", "quantization_level", ...}}]}`.
- Usados: `name` (identificador exibido e enviado de volta ao Ollama), `size` em bytes, `details.parameter_size` e `details.quantization_level` para descrever o modelo.
- `models: []` significa Ollama sem nenhum modelo, e leva ao passo de escolha de modelo do onboarding.

#### `POST /api/show`

- Requisição: `{"model": "<nome>"}`. `404` quando o modelo não está instalado.
- Usados: `capabilities` (lista, por exemplo `completion`, `vision`, `thinking`), o objeto opcional `thinking` (`values` e `default`) e `model_info["<arquitetura>.context_length"]`, que limita o valor máximo de `num_ctx` oferecido na tela.
- Se `capabilities` não incluir `completion` (modelo só de embeddings), o app não o oferece para conversa. **Confirmado na E06 com o Ollama 0.32.9:** `capabilities` traz `completion`, `tools` e `thinking` no `gpt-oss:20b`, e `completion` e `insert` no `deepseek-coder-v2`. A janela de contexto vem em `model_info["<arquitetura>.context_length"]`. Existe também `<arquitetura>.rope.scaling.original_context_length`, que não é a janela e não é usada.

#### `POST /api/pull`

- Requisição: `{"model": "<nome>", "stream": true}`.
- Linhas do stream, na ordem: `{"status": "pulling manifest"}`; várias `{"status": "pulling <digest>", "digest", "total", "completed"}` (`completed` pode faltar antes de o download da camada começar); `verifying sha256 digest`; `writing manifest`; `removing any unused layers`; `{"status": "success"}`.
- **Progresso:** soma de `completed` dividida pela soma de `total` de todos os `digest` já vistos. Até aparecer o primeiro `total`, o progresso é indeterminado.
- **Sucesso** é exclusivamente a linha `success`. Um stream que termina sem ela é download interrompido.
- **Cancelar** é fechar a conexão. A documentação diz que um download cancelado é retomado do ponto onde parou na próxima chamada, o que alimenta a decisão "limpar ou retomar" da E08.
- Modelo inexistente: **confirmado na E06 (Ollama 0.32.9)** que o `/api/pull` responde `HTTP 200` e, no stream, `{"status":"pulling manifest"}` seguido de `{"error":"pull model manifest: file does not exist"}`. Tratado na E08: essa linha (e o `404`) viram `ErroOllama::ModeloInexistente`, com a mensagem amigável. Já `/api/show` e `/api/chat` com modelo inexistente respondem `HTTP 404` e `{"error":"model 'x' not found"}`.

#### `POST /api/chat`

Requisição:

```json
{
  "model": "qwen3:8b",
  "messages": [
    { "role": "system", "content": "<system prompt do projeto + memórias>" },
    { "role": "user", "content": "..." },
    { "role": "assistant", "content": "..." }
  ],
  "stream": true,
  "options": { "temperature": 0.7, "num_ctx": 8192 },
  "keep_alive": "5m"
}
```

Regras:

- **Uma única mensagem `system`, sempre a primeira.** O conteúdo dela é montado a cada envio, nesta ordem: instruções do projeto, memórias globais e memórias do projeto (decidido na E11).
- **Histórico:** só `role` e `content`. O `thinking` de respostas anteriores **não é reenviado**.
- **`think` não é enviado.** O modelo usa o padrão dele, e a UI mostra `thinking` apenas se o modelo o emitir. É isso que dispensa um toggle por conversa.
- **`options`:** só `temperature` e `num_ctx`.
- **`keep_alive`:** no nível superior. Opções na tela: `5m` (padrão do Ollama), `30m`, `1h`, `24h`, "manter carregado" (número `-1`) e "descarregar logo" (número `0`).
- **Cancelar** é fechar a conexão, e o cliente descarta qualquer pedaço que chegar depois. **Confirmado na E10 com o Ollama 0.32.9:** ao desconectar, o Ollama interrompe a geração (o processo `llama-server` saiu de ~90% de GPU para ocioso em menos de 2 s depois do "Parar").

Stream de resposta:

```json
{"model":"qwen3:8b","created_at":"...","message":{"role":"assistant","thinking":"Vou pensar..."},"done":false}
{"model":"qwen3:8b","created_at":"...","message":{"role":"assistant","content":"Olá"},"done":false}
{"model":"qwen3:8b","created_at":"...","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","eval_count":282}
```

- Cada linha traz `message.content` e/ou `message.thinking`. O raciocínio vem antes da resposta. Os dois são acumulados em separado.
- O stream acaba na linha com `done: true`. `done_reason: "length"` indica resposta cortada por limite de tamanho, e a UI avisa. **(a validar na E05.)**
- Sem nenhuma linha com `thinking`, não existe seção de raciocínio, e isso não é erro.

Sugestões de memória (E12): `POST /api/chat` com `"stream": false`, `format` com um JSON Schema da lista de sugestões e a conversa mais a instrução como `messages`. A resposta é lida de `message.content` e validada. Qualquer coisa fora do esquema é erro `RespostaInvalida`. **Confirmado na E12 com o Ollama 0.32.9**, em `qwen3:0.6b` (com raciocínio) e `gemma3:1b` (sem raciocínio): `format` é respeitado nos dois, e `message.content` é sempre um JSON no esquema. Sem `think`, o modelo com raciocínio devolve o `thinking` separado do `content` (que continua válido). `think: false` é aceito por **ambos**, sem erro no modelo que não raciocina, e dispensa o raciocínio no que tem. Por isso a chamada das sugestões envia `think: false`.

#### Erros

Códigos HTTP documentados: `200`, `400` (requisição inválida), `404` (modelo inexistente), `429`, `500` e `502`. Corpo de erro: `{"error": "<texto>"}`. Erro **no meio** de um stream: uma linha `{"error": "<texto>"}`, sem mudar o status HTTP.

Tipos de erro do cliente (E05). O texto final em pt-BR de cada um é o `mensagem_amigavel` do tipo, listado e verificado no catálogo da E14 (`catalogo.rs`).

| Tipo | Quando | O que o usuário vê (resumo) | Recuperação |
| --- | --- | --- | --- |
| `OllamaForaDoAr` | Conexão recusada | O Ollama não está em execução | Oferecer iniciar e tentar de novo |
| `PortaOcupada` | A porta responde, mas não é Ollama | Outro programa está usando a porta do Ollama | Orientar a fechá-lo e tentar de novo; não iniciar nada |
| `VersaoAntiga` | Versão abaixo da mínima | Atualize o Ollama para usar o Memoteca | Aviso, sem bloquear |
| `ModeloNaoEncontrado` | `404` ou erro equivalente | O modelo não está instalado | Oferecer baixar |
| `ModeloSemSuporte` | Sem `completion` | Este modelo não serve para conversa | Escolher outro modelo |
| `TempoEsgotado` | Nenhum byte dentro do limite de ociosidade | O Ollama demorou demais para responder | Tentar de novo |
| `ConexaoInterrompida` | Conexão cai no meio do stream | A conexão com o Ollama caiu | Preservar o texto parcial e oferecer tentar de novo |
| `RespostaInvalida` | JSON inválido ou fora do esquema | Resposta inesperada do Ollama | Preservar o texto parcial |
| `ErroDoOllama` | `4xx` ou `5xx`, ou linha `error` | Não foi possível concluir. Tente de novo | Texto cru só no log |
| Cancelamento | O usuário cancelou | (não é erro para o usuário) | Manter o texto parcial. É devolvido como resultado (`MotivoFim::Cancelada` e `ResultadoDownload::Cancelado`), e não como `ErroOllama` |

Limites de tempo iniciais, ajustáveis (`Limites` no cliente): conexão 3 s (no Windows, conectar a uma porta onde ninguém ouve leva cerca de 2 s até o sistema recusar, e um estouro de tempo durante a conexão também é `OllamaForaDoAr`); `/api/version`, `/api/tags` e `/api/show`, 5 s no total; `/api/chat`, sem limite total e 120 s sem nenhum byte; `/api/pull`, 60 s sem nenhum byte.

#### Ciclo de vida do processo (operações locais)

- **Instalação existente:** procurar `ollama.exe` nas pastas do `PATH` e, depois, em `%LOCALAPPDATA%ProgramsOllama` (local padrão da instalação por usuário). **Confirmado na E06:** na máquina de desenvolvimento o Ollama está instalado em outro disco, e só o `PATH` o encontra. O instalador também registra o `InstallLocation` no registro de desinstalação do usuário, que a E07 pode usar se for preciso.
- **Porta e reuso:** o app testa `GET /api/version`. Se é Ollama, reutiliza e **nunca o encerra**. Se outro programa responde, `PortaOcupada`. Se ninguém responde e há instalação, o app inicia `ollama serve` e passa a ser dono desse processo.
- **O Ollama oficial roda em segundo plano** depois de instalado (ícone de bandeja) e pode iniciar com o Windows. Encontrá-lo já rodando é o caso comum, e ele conta como externo. Um Ollama que o instalador deixe em execução ao terminar também conta como externo. **(a validar na E15, no checklist manual em Windows limpo: se o instalador silencioso deixa o Ollama rodando e se o `PATH` do usuário é atualizado.)**
- **Instalador oficial:** segundo a documentação do Ollama, não exige administrador e instala no diretório do usuário, com pelo menos 4 GB livres para os binários, fora o espaço dos modelos. Parâmetros do modo silencioso, validação e elevação ficam para a E07.

#### Estados de inicialização

| Estado | Detecção | Ação |
| --- | --- | --- |
| Pronto | `/api/version` é Ollama, versão ≥ mínima, há modelos | Abrir o app principal |
| Sem modelos | Ollama pronto, `/api/tags` vazio | Onboarding, passo de modelo |
| Versão antiga | Ollama pronto, versão < mínima | Aviso amigável, uso permitido |
| Instalado, parado | Nada na porta, `ollama.exe` encontrado | Iniciar `ollama serve` e voltar a verificar |
| Não instalado | Nada na porta, `ollama.exe` não encontrado | Onboarding, passo de instalação |
| Porta ocupada | Algo responde na porta e não é Ollama | Erro `PortaOcupada`, nada é iniciado |

### Telas e fluxos

Escrito na E03. Toda a interface em pt-BR. Cada ação aponta para um endpoint do [Contrato Ollama](#contrato-ollama) ou para uma **operação local** (SQLite, arquivo ou processo). "Etapa" é onde a ação passa a existir.

#### Estrutura geral

```
┌───────────────────────────────────────────────────────────────┐
│ Memoteca                            ● Ollama 0.30     Sobre   │
├──────────────────┬────────────────────────────────────────────┤
│ Projetos     [+] │ Projeto: Estudos de Rust          [Editar] │
│ ▾ Estudos Rust   │ ────────────────────────────────────────── │
│    Conversa 1  ● │ Você: ...                                  │
│    Conversa 2    │ ▸ Raciocínio        Modelo: ...            │
│   [+ Conversa]   │                                            │
│ ▸ Receitas       │ ┌────────────────────────────────────────┐ │
│ ──────────────── │ │ Digite sua mensagem…          [Enviar] │ │
│ ◇ Memória global │ └────────────────────────────────────────┘ │
│ ⚙ Configurações  │ Modelo · Temperatura · Contexto · Manter   │
└──────────────────┴────────────────────────────────────────────┘
```

- Janela única, com `single-instance lock`. **Uma conversa ativa por vez.**
- A sidebar lista projetos e, dentro de cada um, as conversas. **O botão "Nova conversa" só existe dentro de um projeto**, e não há conversa avulsa.
- Rodapé da sidebar: Memória global, Configurações. "Sobre" fica no cabeçalho.
- Indicador de status do Ollama no cabeçalho: verde (pronto), amarelo (versão antiga ou iniciando), vermelho (fora do ar ou porta ocupada).

#### T01 — Inicialização

Tela curta enquanto o app verifica o ambiente (estados de inicialização do contrato).

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Verificar o Ollama ao abrir | `GET /api/version`, `GET /api/tags` e detecção local da instalação | E06 |
| Iniciar o Ollama quando instalado e parado | Local: iniciar `ollama serve` | E06 |
| Encaminhar ao onboarding, ao app ou a um erro | Local: estado de inicialização | E06, E08 |

#### T02 — Onboarding

Passos, nesta ordem, com botão "Voltar" quando fizer sentido. Nenhum passo exige terminal.

| Passo | Conteúdo | Ações | Endpoint ou operação | Etapa |
| --- | --- | --- | --- | --- |
| 1. Boas-vindas | O que o Memoteca faz. Nota curta: "O Ollama é um software separado, obtido do site oficial. O Memoteca é independente e não é afiliado a ele." | Continuar | Local | E08 |
| 2. Ollama | Detecta a instalação. Se faltar, explica e oferece instalar, com progresso | Instalar; Tentar de novo | Local: baixar, validar e executar o instalador; `GET /api/version` | E07, E08 |
| 3. Modelo | Catálogo curado (nome amigável, tamanho, RAM/VRAM estimada, descrição) e campo livre para qualquer modelo | Escolher; digitar nome; ver espaço livre | Local: catálogo embutido e espaço em disco | E08 |
| 4. Download | Barra de progresso, tamanho, botão cancelar | Baixar; Cancelar; Tentar de novo | `POST /api/pull`; confirma com `GET /api/tags` | E08 |
| 5. Pronto | Resumo e entrada no app | Começar | Local: marcar onboarding como concluído | E08 |

Falhas previstas, todas com mensagem amigável e botão de recuperação: sem internet, sem espaço em disco, instalador inválido, instalação parcial, download interrompido e modelo inexistente (ver o plano de robustez).

#### T03 — Projeto (criar e editar)

Aberta a partir do botão "+" da sidebar ou de "Editar". Ver o fluxo F3.

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Nome do projeto | Local: salvar projeto | E09 |
| System prompt fixo | Local: salvar projeto | E09 |
| Modelo padrão (lista dos modelos instalados, opcional) | `GET /api/tags`; Local: salvar projeto | E09 |
| Memórias do projeto (atalho para a T06) | Local | E11 |
| Excluir projeto, com confirmação que explica o que será apagado | Local: excluir projeto, conforme a regra da E04 | E09 |

#### T04 — Conversa

Tela principal de uso. Ver o fluxo F4.

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Lista de mensagens, persistida | Local: ler mensagens | E10 |
| Enviar mensagem (Enter), com streaming | `POST /api/chat` | E10 |
| Cancelar geração | Fechar a conexão do `POST /api/chat` | E10 |
| Bloco "Raciocínio" colapsado, só se o modelo emitir `thinking` | Dados do mesmo stream | E10 |
| Regenerar a última resposta (substitui a anterior) | `POST /api/chat`; Local: substituir mensagem | E10 |
| Título da conversa, truncado da primeira mensagem e editável | Local: salvar conversa | E10 |
| Modelo da conversa (herdado do projeto, sobrescrevível) | `GET /api/tags`, `POST /api/show`; Local: salvar conversa | E10 |
| Temperatura, contexto (`num_ctx`) e "manter na memória" (`keep_alive`) | Local: salvar conversa; enviados em `options` e `keep_alive` | E10 |
| Botão "Sugerir memórias desta conversa" | Abre a T07 | E12 |
| Excluir conversa, editar mensagem | **A definir na E10**; a tela não define o comportamento | E10 |
| Renderização de links nas respostas | **A definir na E10** (ver Registro de decisões) | E10 |

Mensagem de erro de envio aparece **dentro da conversa**, abaixo da mensagem do usuário, que nunca é perdida, com o botão "Tentar de novo".

#### T05 — Sidebar

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Listar projetos e conversas | Local: ler projetos e conversas | E09, E10 |
| Selecionar conversa (torna-a a ativa) | Local | E10 |
| Nova conversa (só dentro de um projeto) | Local: criar conversa | E10 |
| Novo projeto | Abre a T03 | E09 |

#### T06 — Memórias

Tela dedicada, com duas abas: **Projeto** (do projeto atual) e **Global**.

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Listar memórias do escopo | Local: ler memórias | E11 |
| Criar memória | Local: salvar memória | E11 |
| Editar memória | Local: salvar memória | E11 |
| Remover memória (silencioso, sem indicador de "contexto mudou") | Local: excluir memória | E11 |

#### T07 — Sugestões de memória

Diálogo aberto pelo botão da T04. Ver o fluxo F6.

| Elemento ou ação | Endpoint ou operação | Etapa |
| --- | --- | --- |
| Gerar sugestões | `POST /api/chat` (`stream: false`, `format`) | E12 |
| Escolher o escopo de cada sugestão ("Deste projeto", padrão, ou "Global") | Local | E12 |
| Aprovar uma sugestão | Local: criar exatamente uma memória | E12 |
| Rejeitar uma sugestão, ou fechar sem decidir | Nada é criado | E12 |

#### T08 — Configurações

| Seção | Ação | Endpoint ou operação | Etapa |
| --- | --- | --- | --- |
| Ollama | Ver estado e versão; reiniciar o servidor iniciado pelo app | `GET /api/version`; Local: processo | E06 |
| Modelos | Ver modelos instalados; baixar outro modelo | `GET /api/tags`, `POST /api/pull` | E08 |
| Dados | Exportar backup | Local: gravar arquivo | E13 |
| Dados | Importar backup, com confirmação | Local: ler arquivo e gravar no banco | E13 |
| Sobre | Abre a T09 | Local | E16 |

#### T09 — Sobre

Texto do aviso de independência (o mesmo do `README.md`):

> **Memoteca** é um projeto independente. Não é afiliado, patrocinado, endossado nem mantido pelo Ollama ou por seus mantenedores, e não tem relação direta com o projeto ou com a marca Ollama. "Ollama" é marca de seus respectivos titulares e aparece aqui apenas para indicar com qual software este aplicativo funciona. O Memoteca não redistribui o Ollama: ele é obtido da fonte oficial e continua sendo software separado, sob a própria licença.

Também mostra a versão do app, a licença (MIT, "Memoteca contributors") e o link do repositório. Sem logotipo, ícone ou identidade visual do Ollama.

#### T10 — Padrões de mensagem de erro

| Padrão | Quando | Exemplo |
| --- | --- | --- |
| Faixa persistente no topo | Estado que afeta o app inteiro | Ollama fora do ar, versão antiga, porta ocupada |
| Diálogo | Falha que bloqueia e exige decisão | Disco insuficiente, banco indisponível, importação inválida |
| Mensagem na conversa | Falha de um envio | Conexão caiu no meio da resposta |
| Aviso breve (toast) | Confirmação ou aviso sem decisão | Backup exportado |

Toda mensagem diz **o que aconteceu** em linguagem simples, **o que fazer** e oferece um botão de ação. Nunca mostra código, texto cru de API nem stack trace. Os detalhes técnicos vão para o log local (`%APPDATA%\io.github.0tter-dev.memoteca\logs\memoteca.log`, até 3 arquivos de 1 MB, definido na E14).

#### Fluxos

**F1 — Abrir o app.** T01 verifica o ambiente → estado "Pronto": abre o app principal na última conversa ativa; "Sem modelos" ou "Não instalado": T02 no passo correspondente; "Instalado, parado": inicia o servidor e verifica de novo; "Versão antiga": abre o app com a faixa de aviso; "Porta ocupada": faixa e diálogo, sem iniciar nada.

**F2 — Primeiro uso (onboarding).** T02, passos 1 a 5. Ao concluir, F1 volta a "Pronto".

**F3 — Criar e gerir projetos.** "+" na sidebar → T03 (nome, system prompt, modelo padrão opcional) → salvar → projeto aparece na sidebar. Editar e excluir (com confirmação) pela T03.

**F4 — Conversar.** Dentro de um projeto, "Nova conversa" → T04 herda o modelo do projeto (ou, sem modelo padrão no projeto, o da primeira conversa criada) → o usuário escreve e envia → o app monta o contexto (system prompt do projeto, mais as memórias a partir da E11) → `POST /api/chat` com streaming → texto aparece aos poucos, com o raciocínio colapsado quando houver → ao terminar, a resposta é gravada. Cancelar mantém o texto parcial. Regenerar substitui a última resposta.

**F5 — Memória manual.** T06 → criar, editar ou remover. Vale para as gerações seguintes e nunca altera mensagens já geradas.

**F6 — Sugerir memórias.** Botão na T04 → T07 chama o modelo uma vez → lista de sugestões → aprovar ou rejeitar cada uma → só as aprovadas viram memória. O histórico da conversa não muda.

**F7 — Exportar e importar.** T08 → Dados → Exportar (escolhe o destino, grava o arquivo) ou Importar (escolhe o arquivo, valida, confirma, aplica de forma atômica).

**F8 — Fechar o app.** O app encerra apenas o Ollama que ele iniciou. Um Ollama externo continua rodando.

**F9 — Qualquer falha previsível.** Mensagem amigável no padrão da T10 e, quando possível, recuperação automática (rollback ou limpeza) ou botão de tentar de novo.

#### Cobertura dos critérios de "MVP pronto"

| Critério | Telas e fluxos |
| --- | --- |
| 1. Onboarding sem terminal até ter um modelo | F1, F2, T01, T02 |
| 2. Projeto, system prompt e memórias manuais | F3, F5, T03, T06 |
| 3. Conversa com modelo, parâmetros e streaming | F4, T04 |
| 4. Sugestões de memória sob demanda | F6, T07 |
| 5. Memórias injetadas (projeto e global) | F4, F5, T04, T06 |
| 6. Exportar e importar | F7, T08 |
| 7. Persistência ao fechar e reabrir | F1, T05 |
| 8. Falhas previsíveis com mensagem amigável | F9, T10 |

### Plano de testes de robustez

Escrito na E03. Cada cenário tem um identificador `Rnn`, e o nome do teste que o cobre cita esse identificador. Na E14, um teste confere que todo `Rnn` desta lista tem ao menos um teste. Os cenários marcados **(falha)** vêm dos critérios de falha das etapas.

#### Estratégia de testes

| Camada | O que testa | Ferramenta | Roda em |
| --- | --- | --- | --- |
| Unidade (Rust) | Regras puras: versão, progresso do pull, montagem de contexto, parser de stream | `cargo test` | `pnpm check` e CI |
| Integração (Rust) | Cliente do Ollama, ciclo de vida, instalador, banco e import/export, contra o servidor falso e abstrações falsas | `cargo test` | `pnpm check` e CI |
| Componente (frontend) | Telas e estados, com os comandos do Tauri simulados | Vitest e Testing Library | `pnpm check` e CI |
| E2E | O app real, do clique ao banco, inclusive fechar e reabrir | Playwright conectado ao WebView2 por CDP | CI, a partir da E15 |
| Manual | Instalação real do Ollama, download real, Windows limpo | Checklist da E15, em VM | Antes do release |

- **Servidor Ollama falso:** um pequeno servidor HTTP local que imita `/api/version`, `/api/tags`, `/api/show`, `/api/pull` e `/api/chat`, com **injeção de falhas** por cenário (conexão recusada, linha partida em dois pedaços, JSON inválido, queda no meio do stream, atraso, `404`, linha `error`, resposta de outro programa). Uma única implementação serve aos testes Rust e ao E2E. A biblioteca HTTP usada nele é escolhida na E05, que avisa o usuário se for uma dependência nova.
- **Abstrações injetáveis** para o que não dá para fazer de verdade em teste: instalador (`Instalador`), espaço livre em disco (`EspacoEmDisco`) e iniciar processo (`Processo`). Os testes usam versões falsas. No E2E, um recurso de compilação de testes troca as reais pelas falsas, e **esse recurso nunca entra no build de release**.
- **E2E por CDP:** o app abre com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<porta>` e o Playwright se conecta a ele. É a técnica já usada para verificar o executável na E01. Ela não é o caminho oficial do Tauri (`tauri-driver`), então a E15 **começa com um teste-piloto** (abrir, interagir, fechar e reabrir). Se o piloto falhar, a alternativa é o `tauri-driver`, e a mudança é registrada no Registro de decisões. **Resultado (E15):** o piloto passou, e a ferramenta continua a mesma. Fechar e reabrir significa encerrar o processo e iniciar outro.
- O servidor falso ocupa a porta `11434` no CI, que não tem Ollama real. Em máquina de desenvolvimento com Ollama real rodando, o E2E se recusa a iniciar para não interferir.

#### Cenários

| ID | Cenário | Como é simulado | Resultado esperado | Etapa do teste |
| --- | --- | --- | --- | --- |
| R01 | Ollama não instalado | Detector de instalação falso devolve "não encontrado" | Estado "Não instalado" e onboarding no passo do Ollama | E06 |
| R02 | Porta `11434` ocupada por outro programa **(falha)** | Servidor falso que responde HTML, JSON sem `version` ou só aceita a conexão | Erro `PortaOcupada`, nenhum processo iniciado, mensagem amigável | E06 |
| R03 | Ollama externo já rodando | Servidor falso na porta, iniciado fora do app | O app o reutiliza, e ao fechar ele continua rodando | E06 |
| R04 | Fechar o app encerra o Ollama que ele iniciou, inclusive em fechamento forçado **(falha)** | Processo falso iniciado pelo app, que depois é finalizado à força | Nenhum processo órfão | E06 |
| R05 | Versão abaixo da mínima | `/api/version` falso devolve `0.29.9` | Faixa de aviso amigável, uso permitido | E06 |
| R06 | Versão com sufixo ou formato inesperado | `0.31.0-rc1`, texto sem versão | Sufixo ignorado; formato inválido vira `PortaOcupada` | E05 |
| R07 | Disco cheio antes de baixar um modelo **(falha)** | `EspacoEmDisco` falso devolve menos que o tamanho do modelo | Download bloqueado ou alertado antes de começar, conforme a regra da E08 | E08 |
| R08 | Disco cheio antes de instalar o Ollama **(falha)** | `EspacoEmDisco` falso devolve menos de 4 GB | Instalação bloqueada antes de começar | E07 |
| R09 | Download de modelo interrompido **(falha)** | Servidor falso fecha a conexão no meio do `/api/pull` | Estado limpo conforme a regra da E08, mensagem amigável e "Tentar de novo" | E08 |
| R10 | Modelo inexistente no campo livre **(falha)** | Servidor falso responde `404` ou linha `error` | Mensagem amigável, sem o texto cru da API | E08 |
| R11 | Sem internet ao baixar o instalador ou o modelo **(falha)** | Download falso falha por conexão; servidor falso recusa a conexão | Mensagem amigável, nada fica pela metade | E07, E08 |
| R12 | Instalação parcial **(falha)** | `Instalador` falso sai com erro ou é interrompido no meio | Rollback, sem resíduo, mensagem e "Tentar de novo" | E07 |
| R13 | Instalador baixado corrompido ou inválido | `Instalador` falso falha na validação | O arquivo não é executado e é removido | E07 |
| R14 | Instalador baixado removido ao fim | Sucesso e falha do `Instalador` falso | O arquivo não existe depois, nos dois casos | E07 |
| R15 | Pedido de elevação do Windows recusado | `Instalador` falso devolve o código de recusa | Mensagem amigável e "Tentar de novo" | E07 |
| R16 | Ollama cai durante o streaming **(falha)** | Servidor falso fecha a conexão no meio do `/api/chat` | `ConexaoInterrompida`, texto parcial e mensagem do usuário preservados, "Tentar de novo" | E05, E10 |
| R17 | Envio com o Ollama fora do ar **(falha)** | Servidor falso desligado | `OllamaForaDoAr`, a mensagem do usuário não é perdida | E10 |
| R18 | Linha do stream partida entre dois pedaços | Servidor falso divide uma linha em dois pedaços | Resposta reconstruída sem perda | E05 |
| R19 | JSON inválido no stream **(falha)** | Servidor falso envia uma linha corrompida | `RespostaInvalida`, parcial preservado | E05 |
| R20 | Stream sem `thinking` | Servidor falso sem campo `thinking` | Sem erro e sem seção de raciocínio | E05, E10 |
| R21 | Cancelar a geração | Cancelar no meio do stream | Conexão fechada e nenhum texto novo depois | E05, E10 |
| R22 | Ollama travado, sem enviar bytes | Servidor falso fica em silêncio além do limite | `TempoEsgotado` | E05 |
| R23 | Modelo removido depois de escolhido | Servidor falso responde `404` no `/api/chat` | `ModeloNaoEncontrado`, com oferta de baixar | E10 |
| R24 | Modelo sem suporte a conversa | `/api/show` falso sem `completion` | `ModeloSemSuporte`, o modelo não é oferecido | E10 |
| R25 | Resposta cortada por limite | Servidor falso termina com `done_reason: "length"` | Aviso de resposta cortada | E10 |
| R26 | Banco indisponível ou corrompido ao abrir **(falha)** | Arquivo de banco travado ou inválido | Diálogo amigável, dados não são apagados e há caminho para o backup | E04, E14 |
| R27 | Migração falha **(falha)** | Migração falsa que falha no meio | Banco como estava e mensagem amigável | E04 |
| R28 | Arquivo de importação inválido, truncado ou de versão desconhecida **(falha)** | Arquivos de teste corrompidos | Recusado e nenhum dado alterado | E13 |
| R29 | Importação falha no meio **(falha)** | Falha injetada durante a gravação | Nada fica importado pela metade | E13 |
| R30 | Exportação sem permissão ou sem espaço | Destino somente leitura ou disco falso cheio | Mensagem amigável e nenhum arquivo parcial deixado | E13 |
| R31 | Sugestão de memória malformada ou vazia **(falha)** | Servidor falso devolve JSON fora do esquema ou vazio | Mensagem amigável e nenhuma memória criada | E12 |
| R32 | Segunda instância do app **(falha)** | Abrir o executável duas vezes | Uma só janela, a existente vem para a frente (verificada na E01) | E15 |
| R33 | Fechar o app durante download ou geração | Fechar no meio | Estado consistente na próxima abertura; o destino do texto parcial é decidido na E10 | E10, E14 |

Os quatro cenários de falha do conceito (disco cheio, porta ocupada, download interrompido e instalação parcial) correspondem a **R07 e R08**, **R02**, **R09** e **R12**.
