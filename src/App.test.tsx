import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
  reabrir = () => Promise.resolve(),
  pasta = () => Promise.resolve(),
}: {
  banco?: Resposta;
  onboarding?: Resposta;
  ollama?: Resposta;
  reabrir?: Resposta;
  pasta?: Resposta;
} = {}) {
  invoke.mockImplementation((comando: string) => {
    switch (comando) {
      case "estado_do_banco":
        return banco();
      case "onboarding_concluido":
        return onboarding();
      case "preparar_ollama":
        return ollama();
      case "tentar_abrir_banco":
        return reabrir();
      case "abrir_pasta_de_dados":
        return pasta();
      case "listar_projetos":
        return Promise.resolve([]);
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
    await screen.findByText("Nenhum projeto ainda");
  });

  it("não usa o nome provisório do projeto", async () => {
    const { container } = render(<App />);
    await screen.findByText("Nenhum projeto ainda");

    expect(container.textContent?.toLowerCase()).not.toContain("ollamaui");
  });

  it("consulta o banco, o onboarding e o Ollama ao abrir", async () => {
    render(<App />);
    await screen.findByText("Nenhum projeto ainda");

    expect(invoke).toHaveBeenCalledWith("estado_do_banco");
    expect(invoke).toHaveBeenCalledWith("onboarding_concluido");
    expect(invoke).toHaveBeenCalledWith("preparar_ollama");
    // A lista de projetos é pedida num efeito, depois de a tela aparecer.
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("listar_projetos"));
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
  });

  it("quem já concluiu vai direto ao app", async () => {
    simularBackend();
    render(<App />);

    expect(await screen.findByText("Nenhum projeto ainda")).toBeVisible();
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

const MENSAGEM_DO_BANCO =
  "Não foi possível abrir os dados do Memoteca. Seus dados não foram apagados.";

describe("R26: os dados não abrem", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("mostra um diálogo amigável com tentar de novo e a pasta dos dados, sem abrir o app", async () => {
    simularBackend({ banco: () => Promise.reject(MENSAGEM_DO_BANCO) });
    render(<App />);

    const dialogo = await screen.findByRole("alertdialog");
    expect(dialogo).toHaveTextContent("Não foi possível abrir seus dados");
    expect(dialogo).toHaveTextContent(MENSAGEM_DO_BANCO);
    expect(dialogo).toHaveTextContent("Seus dados continuam no computador");
    expect(
      screen.getByRole("button", { name: "Tentar de novo" }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", { name: "Abrir a pasta dos dados" }),
    ).toBeEnabled();
    // Sem os dados, o resto do app não aparece.
    expect(screen.queryByText("Nenhum projeto ainda")).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  });

  it("tentar de novo abre os dados sem reiniciar o app e leva ao app", async () => {
    let tentativas = 0;
    simularBackend({
      banco: () => Promise.reject(MENSAGEM_DO_BANCO),
      reabrir: () => {
        tentativas += 1;
        return tentativas === 1
          ? Promise.reject(MENSAGEM_DO_BANCO)
          : Promise.resolve();
      },
    });
    render(<App />);
    await screen.findByRole("alertdialog");

    // A primeira tentativa continua falhando: o diálogo fica, com a mesma mensagem.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    });
    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      MENSAGEM_DO_BANCO,
    );
    expect(
      screen.getByRole("button", { name: "Tentar de novo" }),
    ).toBeEnabled();

    // O problema foi resolvido: a segunda tentativa funciona, na mesma sessão.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    });
    expect(await screen.findByText("Nenhum projeto ainda")).toBeVisible();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(
      invoke.mock.calls.filter(([c]) => c === "tentar_abrir_banco"),
    ).toHaveLength(2);
  });

  it("a tentativa que falha com erro técnico mostra a mensagem padrão, nunca o erro cru", async () => {
    simularBackend({
      banco: () => Promise.reject(MENSAGEM_DO_BANCO),
      reabrir: () =>
        Promise.reject(
          "rusqlite::Error: SQLITE_NOTADB: file is not a database",
        ),
    });
    render(<App />);
    await screen.findByRole("alertdialog");

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    });

    const alerta = await screen.findByRole("alertdialog");
    expect(alerta).toHaveTextContent("Não foi possível verificar os dados");
    expect(alerta).not.toHaveTextContent("SQLITE");
    expect(alerta).not.toHaveTextContent("rusqlite");
  });

  it("abrir a pasta dos dados chama o backend, e uma falha vira mensagem amigável", async () => {
    let falhar = false;
    simularBackend({
      banco: () => Promise.reject(MENSAGEM_DO_BANCO),
      pasta: () =>
        falhar
          ? Promise.reject("Error: os error 2 (explorer.exe)")
          : Promise.resolve(),
    });
    render(<App />);
    await screen.findByRole("alertdialog");

    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "Abrir a pasta dos dados" }),
      );
    });
    expect(
      invoke.mock.calls.filter(([c]) => c === "abrir_pasta_de_dados"),
    ).toHaveLength(1);
    expect(screen.getAllByRole("alert")).toHaveLength(1);

    falhar = true;
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "Abrir a pasta dos dados" }),
      );
    });
    const alertas = await screen.findAllByRole("alert");
    expect(alertas[alertas.length - 1]).toHaveTextContent(
      "Não foi possível abrir a pasta dos dados.",
    );
    expect(alertas[alertas.length - 1]).not.toHaveTextContent("os error");
  });
});

describe("Falha ao verificar o Ollama", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("oferece tentar de novo, e o app segue sem reiniciar quando o Ollama volta", async () => {
    let chamadas = 0;
    simularBackend({
      ollama: () => {
        chamadas += 1;
        return chamadas === 1
          ? Promise.reject(new TypeError("Cannot read properties of undefined"))
          : Promise.resolve(ollamaPronto);
      },
    });
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível verificar o Ollama.");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    });

    expect(await screen.findByText("Nenhum projeto ainda")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
