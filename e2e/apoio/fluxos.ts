import { type Page, expect } from "@playwright/test";
import type { AppEmTeste } from "./app";

/** O modelo que o servidor falso já tem instalado nos testes que não baixam modelo. */
export const MODELO = "falso:1b";

/** Termos que nunca podem aparecer numa mensagem de erro para o usuário. */
const TERMOS_TECNICOS =
  /\b(sqlite|rusqlite|serde|json|http|https|tcp|os error|errno|panic|stack|exception|typeerror|undefined|null|nan|invalid args|command .* not found)\b|\w:\\|\\\\|\.rs\b|\.db\b|:\d{4,5}\/|\b0x[0-9a-f]+\b/i;

/**
 * Confere que cada aviso de erro visível na tela é uma frase amigável em português e não traz
 * texto técnico (SQL, JSON, HTTP, caminhos, códigos). Devolve os textos que viu.
 */
export async function mensagensDeErro(pagina: Page): Promise<string[]> {
  await expect(pagina.getByRole("alert").first()).toBeVisible();
  const textos = (await pagina.getByRole("alert").allInnerTexts())
    .map((texto) => texto.trim())
    .filter((texto) => texto !== "");
  for (const texto of textos) {
    expect(texto, `mensagem de erro técnica: ${texto}`).not.toMatch(
      TERMOS_TECNICOS,
    );
    expect(texto, `mensagem sem pontuação final: ${texto}`).toMatch(
      /[.!?…]$|[.!?…]\s/,
    );
  }
  return textos;
}

/** Passa pelas boas-vindas e pelo passo do Ollama, com o Ollama em execução e um modelo. */
export async function concluirOnboardingComModelo(pagina: Page): Promise<void> {
  await expect(
    pagina.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
  ).toBeVisible();
  await pagina.getByRole("button", { name: "Continuar" }).click();
  await expect(pagina.getByText(/Ollama [\d.]+ em execução\./)).toBeVisible();
  await pagina.getByRole("button", { name: "Continuar" }).click();
  await expect(
    pagina.getByRole("heading", { name: "Tudo pronto" }),
  ).toBeVisible();
  await pagina.getByRole("button", { name: "Começar" }).click();
  await expect(pagina.getByText("Nenhum projeto ainda")).toBeVisible();
}

/** Abre o app, com o Ollama falso já rodando, e vai até a tela principal, vazia. */
export async function abrirAppPronto(app: AppEmTeste): Promise<Page> {
  await app.iniciar();
  await concluirOnboardingComModelo(app.pagina);
  return app.pagina;
}

/** Cria o primeiro projeto pelo botão da tela vazia. */
export async function criarPrimeiroProjeto(
  pagina: Page,
  nome: string,
  instrucoes: string,
): Promise<void> {
  await pagina
    .getByRole("button", { name: "Criar o primeiro projeto" })
    .click();
  await criarProjetoDoFormulario(pagina, nome, instrucoes);
}

/** Cria outro projeto pelo "+" da lateral. */
export async function criarOutroProjeto(
  pagina: Page,
  nome: string,
  instrucoes: string,
): Promise<void> {
  await pagina.getByRole("button", { name: "Novo projeto" }).click();
  await criarProjetoDoFormulario(pagina, nome, instrucoes);
}

async function criarProjetoDoFormulario(
  pagina: Page,
  nome: string,
  instrucoes: string,
): Promise<void> {
  await pagina.getByLabel("Nome do projeto").fill(nome);
  await pagina.getByLabel(/Instruções do projeto/).fill(instrucoes);
  await pagina.getByRole("button", { name: "Criar projeto" }).click();
  await expect(
    pagina.getByRole("heading", { name: nome, level: 2 }),
  ).toBeVisible();
}

/** Abre as memórias do projeto aberto e adiciona uma, na aba pedida. */
export async function adicionarMemoria(
  pagina: Page,
  escopo: "projeto" | "global",
  texto: string,
): Promise<void> {
  if (escopo === "projeto") {
    await pagina.getByRole("button", { name: "Gerenciar memórias" }).click();
    await pagina.getByRole("tab", { name: "Deste projeto" }).click();
  } else {
    await pagina.getByRole("button", { name: "◇ Memória global" }).click();
    await pagina.getByRole("tab", { name: "Globais" }).click();
  }
  await pagina.getByLabel("Nova memória").fill(texto);
  await pagina.getByRole("button", { name: "Adicionar memória" }).click();
  await expect(pagina.getByRole("list", { name: "Memórias" })).toContainText(
    texto,
  );
  await pagina.getByRole("button", { name: "Voltar" }).click();
}

/** Cria uma conversa no projeto aberto e escolhe o modelo dela. */
export async function novaConversaComModelo(
  pagina: Page,
  modelo: string = MODELO,
): Promise<void> {
  await pagina
    .getByRole("button", { name: "Nova conversa", exact: true })
    .click();
  await expect(pagina.getByRole("region", { name: "Conversa" })).toBeVisible();
  await pagina.locator("summary", { hasText: "Modelo e parâmetros" }).click();
  await pagina.getByLabel(/^Modelo/).selectOption(modelo);
  await expect(pagina.getByLabel(/^Modelo/)).toHaveValue(modelo);
}

/** Envia uma mensagem na conversa aberta. */
export async function enviarMensagem(
  pagina: Page,
  texto: string,
): Promise<void> {
  await pagina.getByLabel("Mensagem", { exact: true }).fill(texto);
  await pagina.getByRole("button", { name: "Enviar" }).click();
}

export const RESPOSTA_DO_FALSO = "Olá! Esta é uma resposta de teste.";

/** Espera a resposta completa do servidor falso aparecer na conversa. */
export async function esperarARespostaDoFalso(pagina: Page): Promise<void> {
  await expect(
    pagina.getByRole("article", { name: "Assistente" }).last(),
  ).toContainText(RESPOSTA_DO_FALSO);
}
