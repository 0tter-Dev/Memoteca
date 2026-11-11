import { mkdirSync, copyFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { EXE_DO_OLLAMA_FALSO } from "./apoio/ambiente";
import { expect, test } from "./apoio/fixtures";
import { ControleDoFalso, alguemRespondeNaPorta } from "./apoio/ollama-falso";

// MVP pronto, critério 1: instalar o app, abri-lo e ser guiado pelo onboarding até ter um
// modelo baixado, sem abrir terminal. Aqui o computador não tem Ollama nem modelo: o Ollama é
// "instalado" pelo instalador falso (a instalação real fica no checklist manual).
test("1. sem Ollama e sem modelo, o onboarding instala, baixa um modelo e chega ao app", async ({
  app,
  falsos,
}) => {
  const release = await falsos.release();
  await app.iniciar({ urlDaRelease: release });
  const pagina = app.pagina;

  await expect(
    pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();
  await expect(
    pagina.getByText(/O Memoteca é independente e não é afiliado a ele/),
  ).toBeVisible();
  await pagina.getByRole("button", { name: "Continuar" }).click();

  // O Ollama não está instalado: o Memoteca instala, conferindo o arquivo antes.
  await expect(pagina.getByText(/ainda não está instalado/)).toBeVisible();
  await pagina.getByRole("button", { name: "Instalar o Ollama" }).click();

  // Instalado, o app inicia o Ollama sozinho.
  await expect(pagina.getByText(/Ollama [\d.]+ em execução\./)).toBeVisible();
  await pagina.getByRole("button", { name: "Continuar" }).click();

  // Sem nenhum modelo: escolhe um e baixa, com progresso.
  await expect(
    pagina.getByRole("heading", { name: "Escolha um modelo" }),
  ).toBeVisible();
  await pagina
    .getByRole("textbox", { name: /Outro modelo/ })
    .fill("llama3.2:1b");
  await pagina.getByRole("button", { name: "Baixar" }).click();
  await expect(
    pagina.getByRole("heading", { name: /Baixando llama3.2:1b/ }),
  ).toBeVisible();

  await expect(
    pagina.getByRole("heading", { name: "Tudo pronto" }),
  ).toBeVisible();
  await pagina.getByRole("button", { name: "Começar" }).click();
  await expect(pagina.getByText("Nenhum projeto ainda")).toBeVisible();

  // O app pediu o download ao Ollama que ele mesmo iniciou.
  const controle = new ControleDoFalso();
  const baixados = await controle.corpos("/api/pull");
  expect(baixados.map((corpo) => corpo.model)).toContain("llama3.2:1b");
});

// MVP pronto, critério 1 (e F8): um Ollama que já estava rodando é reaproveitado e não é
// encerrado ao fechar o app, e um que o app iniciou é encerrado junto com ele.
test("1. R03: o Ollama que já rodava continua rodando ao fechar o app", async ({
  app,
  falsos,
}) => {
  await falsos.ollamaExterno([]);
  await app.iniciar();
  await expect(
    app.pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();

  await app.fecharComoOUsuario();

  expect(await alguemRespondeNaPorta(11434)).toBe(true);
});

test("1. R04: o Ollama que o app iniciou é encerrado ao fechar o app", async ({
  app,
  ambiente,
}) => {
  // O Ollama já está instalado (o falso, na pasta padrão), mas não está rodando.
  const pasta = join(ambiente.localAppData, "Programs", "Ollama");
  mkdirSync(pasta, { recursive: true });
  copyFileSync(EXE_DO_OLLAMA_FALSO, join(pasta, "ollama.exe"));
  expect(existsSync(join(pasta, "ollama.exe"))).toBe(true);
  expect(await alguemRespondeNaPorta(11434)).toBe(false);

  await app.iniciar();
  await expect(
    app.pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();
  await app.pagina.getByRole("button", { name: "Continuar" }).click();
  await expect(
    app.pagina.getByText(/Ollama [\d.]+ em execução\./),
  ).toBeVisible();
  expect(await alguemRespondeNaPorta(11434)).toBe(true);

  await app.fecharComoOUsuario();

  await expect
    .poll(() => alguemRespondeNaPorta(11434), { timeout: 10_000 })
    .toBe(false);
});
