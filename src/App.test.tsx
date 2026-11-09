import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const ollamaPronto = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 2,
  aviso: null,
  mensagem: null,
};

type Resposta = () => Promise<unknown>;

/**
 * Simula os comandos do backend. Por padrão: banco ok, onboarding já concluído e Ollama
 * pronto com modelos. Cada argumento troca o que um comando devolve (uma função, para a
 * promessa só nascer quando o comando for chamado).
 */
function simularBackend({
  banco = () => Promise.resolve(),
  onboarding = () => Promise.resolve(true),
  ollama = () => Promise.resolve(ollamaPronto),
}: { banco?: Resposta; onboarding?: Resposta; ollama?: Resposta } = {}) {
  invoke.mockImplementation((comando: string) => {
    switch (comando) {
      case "estado_do_banco":
        return banco();
      case "onboarding_concluido":
        return onboarding();
      case "preparar_ollama":
        return ollama();
      default:
        return Promise.reject(`comando inesperado: ${comando}`);
    }
  });
}

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
    simularBackend();
  });

  it("mostra o nome do projeto como título principal", async () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { level: 1, name: "Memoteca" }),
    ).toBeInTheDocument();
    await screen.findByText("Em construção.");
  });

  it("não usa o nome provisório do projeto", async () => {
    const { container } = render(<App />);
    await screen.findByText("Em construção.");

    expect(container.textContent?.toLowerCase()).not.toContain("ollamaui");
  });

  it("consulta o banco, o onboarding e o Ollama ao abrir", async () => {
    render(<App />);
    await screen.findByText("Em construção.");

    expect(invoke).toHaveBeenCalledWith("estado_do_banco");
    expect(invoke).toHaveBeenCalledWith("onboarding_concluido");
    expect(invoke).toHaveBeenCalledWith("preparar_ollama");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("mostra a mensagem amigável quando o banco não abre", async () => {
    simularBackend({
      banco: () =>
        Promise.reject(
          "Não foi possível abrir os dados do Memoteca. Seus dados não foram apagados.",
        ),
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent(
      "Não foi possível abrir os dados do Memoteca. Seus dados não foram apagados.",
    );
    expect(screen.queryByText("Em construção.")).not.toBeInTheDocument();
  });

  it("mostra uma mensagem amigável, e não o erro cru, quando a consulta do banco falha de outro jeito", async () => {
    simularBackend({
      banco: () =>
        Promise.reject(new TypeError("Cannot read properties of undefined")),
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent(
      "Não foi possível verificar os dados do Memoteca.",
    );
    expect(alerta).not.toHaveTextContent("Cannot read");
  });
});

describe("App e o Ollama", () => {
  beforeEach(() => {
    invoke.mockReset();
    simularBackend();
  });

  it("mostra a versão quando o Ollama está em execução", async () => {
    render(<App />);

    expect(await screen.findByText("Ollama 0.32.9 em execução.")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("mostra o aviso amigável quando a versão está abaixo da mínima, sem bloquear", async () => {
    simularBackend({
      ollama: () =>
        Promise.resolve({
          ...ollamaPronto,
          versao: "0.29.0",
          aviso: "O Ollama instalado é a versão 0.29.0. Atualize o Ollama.",
        }),
    });
    render(<App />);

    expect(await screen.findByText("Ollama 0.29.0 em execução.")).toBeVisible();
    expect(
      screen.getByText(
        "O Ollama instalado é a versão 0.29.0. Atualize o Ollama.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("mostra uma mensagem amigável, e não o erro cru, quando a consulta do Ollama falha", async () => {
    simularBackend({
      ollama: () =>
        Promise.reject(new TypeError("Cannot read properties of undefined")),
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível verificar o Ollama.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });
});

describe("App e o onboarding", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("mostra o onboarding na primeira vez, mesmo com o Ollama e modelos prontos", async () => {
    simularBackend({ onboarding: () => Promise.resolve(false) });
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).toBeVisible();
    expect(screen.queryByText("Em construção.")).not.toBeInTheDocument();
  });

  it("quem já concluiu vai direto ao app", async () => {
    simularBackend();
    render(<App />);

    expect(await screen.findByText("Em construção.")).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).not.toBeInTheDocument();
  });

  it("sem nenhum modelo, volta ao onboarding mesmo para quem já concluiu, sem as boas-vindas", async () => {
    simularBackend({
      ollama: () => Promise.resolve({ ...ollamaPronto, modelosInstalados: 0 }),
    });
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "O Ollama" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).not.toBeInTheDocument();
  });

  it("com o Ollama não instalado, leva ao onboarding com a opção de instalar", async () => {
    simularBackend({
      ollama: () =>
        Promise.resolve({
          estado: "nao_instalado",
          versao: null,
          origem: null,
          modelosInstalados: null,
          aviso: null,
          mensagem: "O Ollama não está instalado neste computador.",
        }),
    });
    render(<App />);

    expect(
      await screen.findByRole("button", { name: "Instalar o Ollama" }),
    ).toBeVisible();
  });

  it("se não conseguir ler o onboarding, mostra o onboarding, que é o lado seguro", async () => {
    simularBackend({ onboarding: () => Promise.reject("falha") });
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).toBeVisible();
  });
});
