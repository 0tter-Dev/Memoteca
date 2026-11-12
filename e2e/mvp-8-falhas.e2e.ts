import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "./apoio/fixtures";
import {
  MODELO,
  RESPOSTA_DO_FALSO,
  abrirAppPronto,
  criarPrimeiroProjeto,
  enviarMensagem,
  esperarARespostaDoFalso,
  mensagensDeErro,
  novaConversaComModelo,
} from "./apoio/fluxos";
import { ocuparAPortaComOutroPrograma } from "./apoio/ollama-falso";

// MVP pronto, critério 8: em qualquer falha previsível, mensagem amigável e, quando possível,
// recuperação sem reiniciar o app. Cada teste provoca a falha no app de verdade, confere que
// a tela só mostra texto amigável e que o app continua utilizável.

test("8. R02: porta do Ollama ocupada por outro programa: avisa e se recupera ao tentar de novo", async ({
  app,
  falsos,
}) => {
  const liberar = await ocuparAPortaComOutroPrograma();
  await app.iniciar();
  const pagina = app.pagina;
  await pagina.getByRole("button", { name: "Continuar" }).click();

  await expect(pagina.getByRole("alert")).toContainText(/porta/);
  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/Feche esse programa e tente de novo/);

  // O usuário fecha o outro programa, e o Ollama passa a responder. Sem reiniciar o app.
  const pid = app.pid;
  await liberar();
  await falsos.ollamaExterno([MODELO]);
  await pagina.getByRole("button", { name: "Verificar de novo" }).click();
  await expect(pagina.getByText(/Ollama [\d.]+ em execução\./)).toBeVisible();
  expect(app.pid).toBe(pid);
  expect(app.aberto).toBe(true);
});

test("8. R09: download interrompido: guarda o que baixou e continua de onde parou", async ({
  app,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([]);
  await app.iniciar();
  const pagina = app.pagina;
  await pagina.getByRole("button", { name: "Continuar" }).click();
  await pagina.getByRole("button", { name: "Continuar" }).click();

  await pagina
    .getByRole("textbox", { name: /Outro modelo/ })
    .fill("llama3.2:1b");
  await falso.cenario("pull_interrompido");
  await pagina.getByRole("button", { name: "Baixar" }).click();

  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/interrompido/);
  await pagina.getByRole("button", { name: "Continuar o download" }).click();

  await expect(
    pagina.getByRole("heading", { name: "Tudo pronto" }),
  ).toBeVisible();
  const baixados = await falso.corpos("/api/pull");
  expect(baixados).toHaveLength(2);
});

test("8. R07: disco cheio: bloqueia o download antes de começar e libera quando há espaço", async ({
  app,
  ambiente,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([]);
  // Só 1 KB livre.
  writeFileSync(join(ambiente.dados, "espaco-livre.txt"), "1000");
  await app.iniciar();
  const pagina = app.pagina;
  await pagina.getByRole("button", { name: "Continuar" }).click();
  await pagina.getByRole("button", { name: "Continuar" }).click();

  await pagina.getByRole("radio", { name: /Llama 3.2/ }).click();
  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/espaço/);
  await expect(pagina.getByRole("button", { name: "Baixar" })).toBeDisabled();
  expect(await falso.corpos("/api/pull")).toHaveLength(0);

  // O usuário libera espaço (100 GB) e escolhe de novo.
  writeFileSync(join(ambiente.dados, "espaco-livre.txt"), "107374182400");
  await pagina.getByRole("radio", { name: /Qwen 3 \(4B\)/ }).click();
  await pagina.getByRole("radio", { name: /Llama 3.2/ }).click();
  await expect(pagina.getByRole("button", { name: "Baixar" })).toBeEnabled();
  await pagina.getByRole("button", { name: "Baixar" }).click();
  await expect(
    pagina.getByRole("heading", { name: "Tudo pronto" }),
  ).toBeVisible();
});

test("8. R11: falha ao instalar o Ollama: mensagem amigável e dá para tentar de novo", async ({
  app,
}) => {
  // Sem Ollama e sem servidor de release: a consulta da instalação não chega a lugar nenhum.
  await app.iniciar();
  const pagina = app.pagina;
  await pagina.getByRole("button", { name: "Continuar" }).click();
  await pagina.getByRole("button", { name: "Instalar o Ollama" }).click();

  await expect(pagina.getByRole("alert")).toBeVisible();
  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/internet|conexão|tente de novo/i);
  await expect(
    pagina.getByRole("button", { name: "Tentar de novo" }),
  ).toBeVisible();

  // O app segue utilizável: tentar de novo roda o fluxo outra vez.
  await pagina.getByRole("button", { name: "Tentar de novo" }).click();
  await expect(pagina.getByRole("alert")).toBeVisible();
  expect(app.aberto).toBe(true);
});

test("8. R16: o Ollama cai no meio da resposta: a conversa guarda o que chegou e dá para tentar de novo", async ({
  app,
  falsos,
}) => {
  const falso = await falsos.ollamaExterno([MODELO]);
  const pagina = await abrirAppPronto(app);
  await criarPrimeiroProjeto(pagina, "Quedas", "");
  await novaConversaComModelo(pagina);

  await falso.cenario("chat_corta");
  await enviarMensagem(pagina, "Conte uma história");
  await expect(pagina.getByRole("alert")).toBeVisible();
  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/conexão|Ollama/);
  // O pedaço que chegou ficou guardado como resposta interrompida.
  await expect(
    pagina.getByRole("article", { name: "Assistente" }),
  ).toContainText("Olá!");

  // O Ollama volta, e tentar de novo termina a resposta.
  await falso.cenario("normal");
  await pagina.getByRole("button", { name: "Tentar de novo" }).click();
  await esperarARespostaDoFalso(pagina);
  await expect(pagina.getByRole("alert")).toHaveCount(0);
  await expect(
    pagina.getByRole("article", { name: "Assistente" }).last(),
  ).toContainText(RESPOSTA_DO_FALSO);
});

// R26: os dados não abrem (arquivo danificado ou que não é um banco). O app não apaga nada,
// mostra um diálogo amigável e volta a funcionar sem reiniciar quando o problema é resolvido.
test("8. R26: os dados não abrem: diálogo amigável, nada é apagado e tentar de novo recupera", async ({
  app,
  ambiente,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  const banco = join(ambiente.dados, "memoteca.db");
  const conteudo = "isto nao e um banco sqlite, mas e do usuario";
  writeFileSync(banco, conteudo);

  await app.iniciar();
  const pagina = app.pagina;
  await expect(pagina.getByRole("alertdialog")).toBeVisible();
  await expect(
    pagina.getByRole("heading", { name: "Não foi possível abrir seus dados" }),
  ).toBeVisible();
  const mensagens = await mensagensDeErro(pagina);
  expect(mensagens.join(" ")).toMatch(/Seus dados não foram apagados/);
  // O detalhe técnico vai só para o log, e o arquivo do usuário continua como estava.
  expect(app.log()).toContain("banco");
  expect(readFileSync(banco, "utf8")).toBe(conteudo);

  // Tentar de novo com o problema ainda lá: o diálogo continua, e o arquivo também.
  await pagina.getByRole("button", { name: "Tentar de novo" }).click();
  await expect(pagina.getByRole("alertdialog")).toBeVisible();
  expect(readFileSync(banco, "utf8")).toBe(conteudo);

  // O usuário resolve o problema (aqui, tirando o arquivo do lugar) e tenta de novo.
  const pid = app.pid;
  rmSync(banco);
  await pagina.getByRole("button", { name: "Tentar de novo" }).click();
  await expect(
    pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();
  expect(app.pid).toBe(pid);
  expect(existsSync(banco)).toBe(true);
});

// R32: abrir o executável uma segunda vez não abre outra janela: a que já existe vem para a
// frente.
test("8. R32: uma segunda instância do app não abre outra janela e traz a existente para a frente", async ({
  app,
  falsos,
}) => {
  await falsos.ollamaExterno([MODELO]);
  await app.iniciar();
  await expect(
    app.pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();
  const pid = app.pid;

  // A janela existente está minimizada na barra de tarefas.
  app.minimizarJanela();
  await expect.poll(() => app.janelaMinimizada()).toBe(true);

  const saida = await app.abrirSegundaInstancia();

  // A segunda execução termina sozinha, a primeira continua e é a única, e a janela volta.
  expect(saida, "a segunda instância não terminou").not.toBeNull();
  expect(app.aberto).toBe(true);
  expect(app.pid).toBe(pid);
  expect(app.quantosProcessosDoApp()).toBe(1);
  await expect.poll(() => app.janelaMinimizada()).toBe(false);
});
