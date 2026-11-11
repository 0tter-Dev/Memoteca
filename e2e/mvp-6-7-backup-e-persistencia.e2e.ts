import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { AppEmTeste } from "./apoio/app";
import { apagarAmbiente, criarAmbiente } from "./apoio/ambiente";
import { expect, test } from "./apoio/fixtures";
import {
  MODELO,
  abrirAppPronto,
  adicionarMemoria,
  concluirOnboardingComModelo,
  criarPrimeiroProjeto,
  enviarMensagem,
  esperarARespostaDoFalso,
  mensagensDeErro,
  novaConversaComModelo,
} from "./apoio/fluxos";

// MVP pronto, critério 7: fechar e reabrir o app e encontrar tudo persistido. O app é
// encerrado de verdade e outro processo é iniciado (o `pid` muda).
test("7. fechar e reabrir o app: projetos, conversas, memórias e ajustes continuam lá", async ({
  app,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);

  await criarPrimeiroProjeto(pagina, "Persistência", "Seja breve.");
  await adicionarMemoria(pagina, "projeto", "Usa Rust 2024");
  await adicionarMemoria(pagina, "global", "Fala português");
  await novaConversaComModelo(pagina);
  await pagina.getByLabel(/Temperatura/).fill("0.4");
  await expect(pagina.getByLabel(/Temperatura/)).toHaveValue("0.4");
  await enviarMensagem(pagina, "Lembre-se disto");
  await esperarARespostaDoFalso(pagina);

  const { antes, depois } = await app.reiniciar();
  expect(depois).not.toBe(antes);

  // O onboarding não volta, e o que o usuário criou continua no lugar.
  const reaberta = app.pagina;
  await expect(
    reaberta.getByRole("navigation", { name: "Lista de projetos" }),
  ).toContainText("Persistência");
  await expect(reaberta.getByText("Bem-vindo ao Memoteca")).toHaveCount(0);

  await reaberta.getByRole("button", { name: "Persistência" }).click();
  await expect(reaberta.getByText("Seja breve.")).toBeVisible();
  await reaberta.getByRole("button", { name: "Gerenciar memórias" }).click();
  await expect(reaberta.getByRole("list", { name: "Memórias" })).toContainText(
    "Usa Rust 2024",
  );
  await reaberta.getByRole("tab", { name: "Globais" }).click();
  await expect(reaberta.getByRole("list", { name: "Memórias" })).toContainText(
    "Fala português",
  );
  await reaberta.getByRole("button", { name: "Voltar" }).click();

  await reaberta
    .getByRole("list", { name: "Conversas de Persistência" })
    .getByRole("button")
    .first()
    .click();
  await expect(reaberta.getByRole("article", { name: "Você" })).toContainText(
    "Lembre-se disto",
  );
  await esperarARespostaDoFalso(reaberta);
  await reaberta.locator("summary", { hasText: "Modelo e parâmetros" }).click();
  await expect(reaberta.getByLabel(/^Modelo/)).toHaveValue(MODELO);
  await expect(reaberta.getByLabel(/Temperatura/)).toHaveValue("0.4");
});

// MVP pronto, critério 6: exportar e importar dados como backup manual. O backup sai do
// primeiro app e entra num segundo app, com dados novos e vazios.
test("6. exporta um backup e o importa em outro app, sem apagar nada", async ({
  app,
  ambiente,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  let pagina = await abrirAppPronto(app);
  await criarPrimeiroProjeto(pagina, "Para levar", "Instruções do projeto.");
  await adicionarMemoria(pagina, "projeto", "Usa Rust 2024");
  await adicionarMemoria(pagina, "global", "Fala português");
  await novaConversaComModelo(pagina);
  await enviarMensagem(pagina, "Mensagem a guardar");
  await esperarARespostaDoFalso(pagina);

  // Exportar: o usuário "escolhe" o destino, e o arquivo é gravado.
  await pagina.getByRole("button", { name: "⚙ Configurações" }).click();
  await pagina.getByRole("button", { name: "Exportar backup…" }).click();
  await expect(pagina.getByText(/Backup salvo em/)).toBeVisible();

  const arquivos = readdirSync(ambiente.escolhas).filter((nome) =>
    nome.endsWith(".json"),
  );
  expect(arquivos).toHaveLength(1);
  const caminho = join(ambiente.escolhas, arquivos[0]);
  const conteudo = readFileSync(caminho, "utf8");
  expect(JSON.parse(conteudo).formato).toBe("memoteca-backup");
  expect(conteudo).toContain("Mensagem a guardar");
  expect(conteudo).toContain("Fala português");

  // Outro computador: dados novos e vazios.
  await app.fecharComoOUsuario();
  const outroAmbiente = criarAmbiente();
  const outro = new AppEmTeste(outroAmbiente);
  try {
    await outro.iniciar();
    pagina = outro.pagina;
    await concluirOnboardingComModelo(pagina);

    writeFileSync(outro.arquivoParaImportar, conteudo);
    await pagina.getByRole("button", { name: "⚙ Configurações" }).click();
    await pagina.getByRole("button", { name: "Importar backup…" }).click();
    await expect(pagina.getByText("Importar este backup?")).toBeVisible();
    await pagina.getByRole("button", { name: "Importar", exact: true }).click();
    await expect(pagina.getByText(/Backup importado/)).toBeVisible();

    // Tudo chegou: o projeto, a memória do projeto, a global e a conversa com as mensagens.
    await expect(
      pagina.getByRole("navigation", { name: "Lista de projetos" }),
    ).toContainText("Para levar");
    await pagina.getByRole("button", { name: "◇ Memória global" }).click();
    await expect(pagina.getByRole("list", { name: "Memórias" })).toContainText(
      "Fala português",
    );
    await pagina.getByRole("button", { name: "Voltar" }).click();
    await pagina.getByRole("button", { name: "Para levar" }).click();
    await pagina
      .getByRole("list", { name: "Conversas de Para levar" })
      .getByRole("button")
      .first()
      .click();
    await expect(pagina.getByRole("article", { name: "Você" })).toContainText(
      "Mensagem a guardar",
    );

    // Importar de novo não duplica nada.
    await pagina.getByRole("button", { name: "⚙ Configurações" }).click();
    await pagina.getByRole("button", { name: "Importar backup…" }).click();
    await expect(pagina.getByRole("button", { name: "Fechar" })).toBeVisible();
    await expect(
      pagina.getByRole("button", { name: "Importar", exact: true }),
    ).toHaveCount(0);
    await pagina.getByRole("button", { name: "Fechar" }).click();
  } finally {
    await outro.matar();
    apagarAmbiente(outroAmbiente);
  }
});

// MVP pronto, critério 8 (falha) e R28: um arquivo que não é um backup é recusado com uma
// mensagem amigável, e nada muda.
test("6. R28: um arquivo que não é um backup é recusado com mensagem amigável", async ({
  app,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);
  await criarPrimeiroProjeto(pagina, "Intacto", "");

  writeFileSync(
    app.arquivoParaImportar,
    '{"formato":"outro","segredo":"SEGREDO-DO-ARQUIVO"}',
  );
  await pagina.getByRole("button", { name: "⚙ Configurações" }).click();
  await pagina.getByRole("button", { name: "Importar backup…" }).click();

  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/não é um backup do Memoteca/);
  // O conteúdo do arquivo nunca vai para a tela, nem para o log.
  await expect(pagina.getByText("SEGREDO-DO-ARQUIVO")).toHaveCount(0);
  expect(app.log()).not.toContain("SEGREDO-DO-ARQUIVO");
  expect(existsSync(app.arquivoParaImportar)).toBe(true);

  await expect(
    pagina.getByRole("navigation", { name: "Lista de projetos" }),
  ).toContainText("Intacto");
});
