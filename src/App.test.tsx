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

/** Simula os comandos do backend: `banco` e `ollama` são o que cada um devolve ou rejeita. */
function simularBackend({
  banco = Promise.resolve(),
  ollama = Promise.resolve(ollamaPronto),
}: { banco?: Promise<unknown>; ollama?: Promise<unknown> } = {}) {
  invoke.mockImplementation((comando: string) =>
    comando === "estado_do_banco" ? banco : ollama,
  );
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

  it("consulta o estado do banco ao abrir e não mostra erro quando está tudo certo", async () => {
    render(<App />);
    await screen.findByText("Em construção.");

    expect(invoke).toHaveBeenCalledWith("estado_do_banco");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("mostra a mensagem amigável quando o banco não abre", async () => {
    simularBackend({
      banco: Promise.reject(
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
      banco: Promise.reject(
        new TypeError("Cannot read properties of undefined"),
      ),
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

  it("prepara o Ollama ao abrir e mostra a versão quando está em execução", async () => {
    render(<App />);

    expect(await screen.findByText("Ollama 0.32.9 em execução.")).toBeVisible();
    expect(invoke).toHaveBeenCalledWith("preparar_ollama");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Nenhum modelo instalado ainda."),
    ).not.toBeInTheDocument();
  });

  it("mostra o aviso amigável quando a versão está abaixo da mínima, sem bloquear", async () => {
    simularBackend({
      ollama: Promise.resolve({
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

  it("avisa quando ainda não há nenhum modelo instalado", async () => {
    simularBackend({
      ollama: Promise.resolve({ ...ollamaPronto, modelosInstalados: 0 }),
    });
    render(<App />);

    expect(
      await screen.findByText("Nenhum modelo instalado ainda."),
    ).toBeVisible();
  });

  it.each([
    ["nao_instalado", "O Ollama não está instalado neste computador."],
    ["porta_ocupada", "Outro programa está usando a porta do Ollama (11434)."],
    ["indisponivel", "Não foi possível iniciar o Ollama."],
  ])("mostra a mensagem amigável no estado %s", async (estado, mensagem) => {
    simularBackend({
      ollama: Promise.resolve({
        estado,
        versao: null,
        origem: null,
        modelosInstalados: null,
        aviso: null,
        mensagem,
      }),
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent(mensagem);
    expect(screen.queryByText(/em execução/)).not.toBeInTheDocument();
  });

  it("mostra uma mensagem amigável, e não o erro cru, quando a consulta do Ollama falha", async () => {
    simularBackend({
      ollama: Promise.reject(
        new TypeError("Cannot read properties of undefined"),
      ),
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível verificar o Ollama.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });
});

describe("App e a instalação do Ollama", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("oferece instalar o Ollama quando ele não está instalado", async () => {
    simularBackend({
      ollama: Promise.resolve({
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
    expect(
      screen.getByText("O Ollama não está instalado neste computador."),
    ).toBeVisible();
  });

  it("não oferece instalar quando o Ollama já está em execução", async () => {
    simularBackend();
    render(<App />);
    await screen.findByText("Ollama 0.32.9 em execução.");

    expect(
      screen.queryByRole("button", { name: "Instalar o Ollama" }),
    ).not.toBeInTheDocument();
  });
});
