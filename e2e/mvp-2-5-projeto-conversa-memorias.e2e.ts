import { expect, test } from "./apoio/fixtures";
import {
  MODELO,
  RESPOSTA_DO_FALSO,
  abrirAppPronto,
  adicionarMemoria,
  criarOutroProjeto,
  criarPrimeiroProjeto,
  enviarMensagem,
  esperarARespostaDoFalso,
  novaConversaComModelo,
} from "./apoio/fluxos";

// MVP pronto, critério 2: criar um projeto, definir o system prompt e adicionar memórias
// manualmente.
test("2. cria um projeto com instruções e adiciona memórias do projeto e globais", async ({
  app,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);

  await criarPrimeiroProjeto(
    pagina,
    "Estudos de Rust",
    "Responda em português.",
  );
  await expect(pagina.getByText("Responda em português.")).toBeVisible();
  await expect(
    pagina.getByRole("navigation", { name: "Lista de projetos" }),
  ).toContainText("Estudos de Rust");

  await pagina.getByRole("button", { name: "Gerenciar memórias" }).click();
  await pagina.getByLabel("Nova memória").fill("Usa Rust 2024");
  await pagina.getByRole("button", { name: "Adicionar memória" }).click();
  await expect(pagina.getByRole("list", { name: "Memórias" })).toContainText(
    "Usa Rust 2024",
  );

  await pagina.getByRole("tab", { name: "Globais" }).click();
  await pagina.getByLabel("Nova memória").fill("Fala português");
  await pagina.getByRole("button", { name: "Adicionar memória" }).click();
  await expect(pagina.getByRole("list", { name: "Memórias" })).toContainText(
    "Fala português",
  );

  // Cada memória fica no seu escopo.
  await pagina.getByRole("tab", { name: "Deste projeto" }).click();
  const doProjeto = pagina.getByRole("list", { name: "Memórias" });
  await expect(doProjeto).toContainText("Usa Rust 2024");
  await expect(doProjeto).not.toContainText("Fala português");
});

// MVP pronto, critério 3: abrir uma conversa no projeto, escolher modelo e parâmetros
// (`temperature`, `num_ctx`, `keep_alive`) e conversar com streaming.
test("3. conversa com o modelo escolhido, com os parâmetros e com streaming", async ({
  app,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);
  await criarPrimeiroProjeto(pagina, "Conversas", "");
  await novaConversaComModelo(pagina);

  // Cada ajuste é gravado na conversa. O campo só mostra o valor novo depois de gravado, então
  // esperar por ele evita que o ajuste seguinte leia a conversa antes da gravação.
  await pagina.getByLabel(/Temperatura/).fill("0.5");
  await expect(pagina.getByLabel(/Temperatura/)).toHaveValue("0.5");
  await pagina.getByLabel(/Contexto/).selectOption("4096");
  await expect(pagina.getByLabel(/Contexto/)).toHaveValue("4096");
  await pagina.getByLabel(/Manter o modelo/).selectOption("30m");
  await expect(pagina.getByLabel(/Manter o modelo/)).toHaveValue("30m");

  await enviarMensagem(pagina, "Oi, tudo bem?");

  // A resposta chega aos poucos: primeiro um pedaço, depois a resposta inteira.
  const resposta = pagina.getByRole("article", { name: "Assistente" });
  await expect(pagina.getByText("Gerando a resposta…")).toBeVisible();
  await esperarARespostaDoFalso(pagina);
  await expect(resposta).toHaveCount(1);
  await expect(pagina.getByRole("article", { name: "Você" })).toContainText(
    "Oi, tudo bem?",
  );

  // O pedido ao Ollama levou o modelo e os parâmetros escolhidos, em streaming.
  const pedidos = await falso.corpos("/api/chat");
  expect(pedidos).toHaveLength(1);
  const pedido = pedidos[0] as {
    model: string;
    stream: boolean;
    keep_alive: string;
    options: { temperature: number; num_ctx: number };
  };
  expect(pedido.model).toBe(MODELO);
  expect(pedido.stream).toBe(true);
  expect(pedido.options.temperature).toBe(0.5);
  expect(pedido.options.num_ctx).toBe(4096);
  expect(pedido.keep_alive).toBe("30m");
});

// MVP pronto, critério 5: as memórias do projeto são injetadas nas conversas dele, e as
// globais em qualquer conversa de qualquer projeto.
test("5. as memórias do projeto e as globais entram no contexto, cada uma onde vale", async ({
  app,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);

  await criarPrimeiroProjeto(pagina, "Projeto A", "Seja gentil.");
  await adicionarMemoria(pagina, "projeto", "Usa Rust 2024");
  await adicionarMemoria(pagina, "global", "Fala português");
  await novaConversaComModelo(pagina);
  await enviarMensagem(pagina, "Pergunta A");
  await esperarARespostaDoFalso(pagina);

  // Outro projeto, outra conversa: a memória global vale, a do projeto A não.
  await criarOutroProjeto(pagina, "Projeto B", "");
  await novaConversaComModelo(pagina);
  await enviarMensagem(pagina, "Pergunta B");
  await expect(
    pagina.getByRole("article", { name: "Assistente" }).last(),
  ).toContainText(RESPOSTA_DO_FALSO);

  const pedidos = (await falso.corpos("/api/chat")) as {
    messages: { role: string; content: string }[];
  }[];
  expect(pedidos).toHaveLength(2);

  const sistemaA = pedidos[0].messages[0];
  expect(sistemaA.role).toBe("system");
  expect(sistemaA.content).toContain("Seja gentil.");
  expect(sistemaA.content).toContain("Fala português");
  expect(sistemaA.content).toContain("Usa Rust 2024");
  // Ordem decidida: instruções do projeto, memórias globais, memórias do projeto.
  expect(sistemaA.content.indexOf("Seja gentil.")).toBeLessThan(
    sistemaA.content.indexOf("Fala português"),
  );
  expect(sistemaA.content.indexOf("Fala português")).toBeLessThan(
    sistemaA.content.indexOf("Usa Rust 2024"),
  );

  const sistemaB = pedidos[1].messages[0];
  expect(sistemaB.content).toContain("Fala português");
  expect(sistemaB.content).not.toContain("Usa Rust 2024");
  expect(sistemaB.content).not.toContain("Seja gentil.");
});

// MVP pronto, critério 4: sugestões de memória sob demanda, aprovadas ou rejeitadas uma a uma.
test("4. sugere memórias sob demanda: nada vira memória sem aprovação", async ({
  app,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);
  await criarPrimeiroProjeto(pagina, "Sugestões", "");
  await novaConversaComModelo(pagina);
  await enviarMensagem(
    pagina,
    "Eu trabalho com Rust e gosto de respostas curtas.",
  );
  await esperarARespostaDoFalso(pagina);

  // Só pede quando o usuário aperta o botão: até aqui, nenhuma chamada extra.
  const extras = async () =>
    (await falso.corpos("/api/chat")).filter((corpo) => corpo.stream === false);
  expect(await extras()).toHaveLength(0);

  // O modelo propõe. Fechar sem decidir não guarda nada.
  await pagina.getByRole("button", { name: "Sugerir memórias" }).click();
  const dialogo = pagina.getByRole("dialog");
  await expect(
    dialogo.getByText("O usuário trabalha com Rust.", { exact: true }),
  ).toBeVisible();
  await expect(
    dialogo.getByText("O usuário prefere respostas curtas.", { exact: true }),
  ).toBeVisible();
  await dialogo.getByRole("button", { name: "Fechar" }).click();
  await expect(dialogo).toBeHidden();
  expect(await extras()).toHaveLength(1);

  await pagina.getByRole("button", { name: "◇ Memória global" }).click();
  await expect(pagina.getByText("Nenhuma memória ainda.")).toBeVisible();
  await pagina.getByRole("button", { name: "Sugestões", exact: true }).click();
  await pagina.getByRole("button", { name: "Gerenciar memórias" }).click();
  await expect(pagina.getByText("Nenhuma memória ainda.")).toBeVisible();

  // De volta à conversa, aprova uma sugestão (deste projeto) e rejeita a outra.
  await pagina.getByRole("button", { name: "Voltar" }).click();
  await pagina
    .getByRole("list", { name: "Conversas de Sugestões" })
    .getByRole("button")
    .first()
    .click();
  await pagina.getByRole("button", { name: "Sugerir memórias" }).click();
  await dialogo
    .getByRole("button", {
      name: /Aprovar a sugestão “O usuário trabalha com Rust\.”/,
    })
    .click();
  await dialogo
    .getByRole("button", {
      name: /Rejeitar a sugestão “O usuário prefere respostas curtas\.”/,
    })
    .click();
  await expect(dialogo.getByText("Pronto: 1 memória guardada.")).toBeVisible();
  await dialogo.getByRole("button", { name: "Fechar" }).click();

  // Só a aprovada virou memória do projeto.
  await pagina.getByRole("button", { name: "Sugestões", exact: true }).click();
  await pagina.getByRole("button", { name: "Gerenciar memórias" }).click();
  const memorias = pagina.getByRole("list", { name: "Memórias" });
  await expect(memorias).toContainText("O usuário trabalha com Rust.");
  await expect(memorias).not.toContainText("prefere respostas curtas");
});
