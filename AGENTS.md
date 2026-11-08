# AGENTS.md

## Antes de mudar algo

Leia o `README.md` e o `PLANO.md`. Eles são a fonte de verdade sobre escopo, regras de negócio, ordem de trabalho e decisões do projeto. Não há outra documentação.

## Como o trabalho é conduzido

- Trabalhe **somente na etapa que o usuário mandou executar**. Não inicie nem adiante outra etapa por conta própria.
- Cada etapa vira **um PR**. Implemente, valide, atualize a documentação, faça o commit e abra o PR, sem pedir nova autorização para essa sequência.
- **Nunca faça merge da própria PR.** O merge é sempre do usuário.
- Cumpra os **Critérios de aceite** da etapa, inclusive os de falha, com testes automatizados. Quando um critério não puder ser automatizado, diga isso no PR e registre-o no checklist manual da E15.
- Os checks do GitHub precisam estar verdes antes de pedir revisão. Não desative, afrouxe nem contorne um check para passar.

## Itens "a definir" e mudanças de escopo

- Não preencha por suposição um item marcado como **a definir** no `PLANO.md`. Proponha uma opção ao usuário, espere a decisão e registre-a no Registro de decisões, no mesmo PR.
- Não mude o escopo, as regras de negócio nem a stack descritos no `PLANO.md` e no `README.md` sem aprovação explícita do usuário.
- Não adicione dependências novas sem avisar o usuário, e prefira as já decididas.

## Documentação no mesmo PR

Atualize no próprio PR da etapa:

- a coluna **Status** do quadro de etapas e o Registro de decisões, no `PLANO.md`;
- o `README.md` (Quick Start, Escopo atual, Status e Stack) quando a mudança alterar o que o projeto roda, oferece ou usa.

## Convenções

- Commits seguem **Conventional Commits** (`feat:`, `fix:`, `docs:`, `chore:`, `test:`…).
- Branches: `<tipo>/<etapa>-<slug>` (ex.: `feat/e04-persistencia`).
- Mudanças pequenas, explícitas e revisáveis.
- Toda a interface e a documentação ficam em **pt-BR**. Mensagens de erro ao usuário são sempre amigáveis, nunca erros técnicos crus.
- Nunca grave segredos, chaves ou credenciais no repositório.

## Independência em relação ao Ollama

O projeto se chama **Memoteca**. Ele **não tem relação direta** com o Ollama (projeto ou marca) e não é afiliado a ele. O nome `OllamaUI` foi apenas provisório e não deve ser usado em código, pacotes, instalador, documentação nem commits.

- Não use logotipo, ícone, imagens nem identidade visual do Ollama.
- Textos, mensagens e documentação nunca sugerem que o app é oficial, do Ollama ou endossado por ele. Cite "Ollama" apenas para descrever compatibilidade.
- O aviso de independência do `README.md` e do `PLANO.md` deve ser mantido, e levado à tela "Sobre" e ao instalador nas etapas previstas.
- Não use `Ollama` em nomes de pacotes, identificadores, domínios ou publicações do projeto. O nome do projeto é Memoteca, e "Ollama" só aparece para descrever compatibilidade.
- Não redistribua o Ollama (binários, instalador ou modelos) dentro do app ou do repositório.

## Regras de segurança

- Não encerre nem altere um Ollama que o app não iniciou.
- Não execute binários baixados sem a validação definida na etapa E07.
- Não apague dados do usuário nem arquivos fora do escopo da etapa.
- Não enfraqueça os fluxos de aprovação: nada vira memória sem aprovação explícita do usuário.
