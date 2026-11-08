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
| E02 | Verificações automáticas no GitHub (CI) | E01 | em revisão |
| E03 | Contrato com a API do Ollama, telas e fluxos | E01 | pendente |
| E04 | Persistência SQLite e modelo de dados | E01 | pendente |
| E05 | Cliente da API do Ollama (HTTP e streaming) | E03 | pendente |
| E06 | Ciclo de vida do Ollama | E05 | pendente |
| E07 | Instalação silenciosa do Ollama | E06 | pendente |
| E08 | Onboarding e catálogo de modelos | E07 | pendente |
| E09 | Projetos | E03, E04 | pendente |
| E10 | Conversas e chat com streaming | E05, E09 | pendente |
| E11 | Memória manual (projeto e global) e injeção | E10 | pendente |
| E12 | Sugestão de memórias sob demanda | E11 | pendente |
| E13 | Exportar e importar | E04, E11 | pendente |
| E14 | Robustez transversal: erros amigáveis e recuperação | E07, E08, E10, E13 | pendente |
| E15 | Testes funcionais ponta a ponta do MVP | E14 | pendente |
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

**Decisões a tomar:** versão mínima do Ollama, bibliotecas de UI (componentes, estado, markdown, realce de código) e ferramenta de E2E.

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

**Decisões a tomar:** biblioteca SQLite (`rusqlite`, `sqlx`…), formato do ID (UUID, ULID…), exclusão física ou lógica (tombstones) pensando no sync futuro, o que acontece com conversas e memórias ao excluir um projeto, e backup do banco antes de migrar.

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

**Decisões a tomar:** versão mínima exata (provavelmente ligada ao suporte ao campo `thinking`, a confirmar) e mecanismo de encerramento garantido (um Job Object do Windows é a sugestão).

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

**Decisões a tomar:** origem e parâmetros exatos do instalador silencioso, como o instalador é validado (assinatura digital, hash…) e como tratar o pedido de elevação de privilégio do Windows.

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

**Decisões a tomar:** lista inicial de modelos do catálogo, margem de espaço que diferencia alerta de bloqueio, e se um download interrompido é limpo ou retomado.

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

**Decisões a tomar:** tamanho do truncamento do título, e comportamento de excluir conversa e editar mensagem, que o conceito não cobre.

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

**Decisões a tomar:** ordem de montagem do contexto (system prompt, global, projeto) e o que fazer quando o contexto montado se aproxima do `num_ctx`.

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

**Decisões a tomar:** qual modelo gera as sugestões e se o escopo (projeto ou global) é escolhido na aprovação.

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

**Decisões a tomar:** formato (JSON ou Markdown, ou os dois) e política de importação (mesclar ou substituir).

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

**Decisões a tomar:** ferramenta de E2E para o Tauri no Windows (a definir na E03).

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
| Cobertura de testes no CI | Não exigida por enquanto, sem ferramenta de cobertura nem dependência nova. O código ainda é mínimo e um percentual não diria nada. Reavaliar na E15, com o código do MVP pronto; se entrar, deve entrar também no `pnpm check` | 2026-11-08 |
| Significado de "build" no CI | O CI roda só `pnpm check`. O build é o do frontend (`tsc && vite build`, parte do `pnpm check`), e o Rust é compilado por `clippy` e `cargo test`. O executável (`tauri build --no-bundle`, com LTO) não roda no CI por custo, e o build do instalador entra na E16 | 2026-11-08 |

### Pendentes

| Decisão | Etapa que a resolve |
| --- | --- |
| Versão mínima do Ollama | E03 e E06 |
| Bibliotecas de UI (componentes, estado, markdown, realce de código) | E03 |
| Ferramenta de E2E para o Tauri | E03 e E15 |
| Biblioteca SQLite e estratégia de migração | E04 |
| Formato dos IDs | E04 |
| Exclusão física ou lógica (tombstones) | E04 |
| O que acontece com as conversas e memórias ao excluir um projeto | E04 e E09 |
| Backup do banco antes de migrar | E04 |
| Mecanismo de encerramento garantido do Ollama iniciado pelo app | E06 |
| Origem, parâmetros e validação do instalador do Ollama; elevação de privilégio | E07 |
| Lista inicial do catálogo de modelos | E08 |
| Margem de espaço (alerta ou bloqueio) | E08 |
| Download interrompido: limpar ou retomar | E08 |
| Tamanho do truncamento do título | E10 |
| Excluir conversa e editar mensagem | E10 |
| Ordem de montagem do contexto e limite do `num_ctx` | E11 |
| Modelo e escopo das sugestões de memória | E12 |
| Formato do export e política de importação | E13 |
| Formato do instalador, assinatura de código, dados na desinstalação | E16 |
| Identidade visual (logo e paleta). Opcional, a discutir quando o usuário quiser, com a skill `project-identity`. Direção inicial: marca geométrica de "prateleiras e um ponto". Os ícones atuais são os padrões do Tauri e são provisórios | Antes de E16 |
| Consulta formal de registro de marca (INPI, USPTO, EUIPO) e do domínio, antes de divulgar amplamente. A triagem feita não substitui essa consulta | Mantenedor, antes de divulgar |

### Contrato Ollama

_A preencher na E03._

### Telas e fluxos

_A preencher na E03._

### Plano de testes de robustez

_A preencher na E03._
