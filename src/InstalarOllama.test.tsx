import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import InstalarOllama from "./InstalarOllama";

const { invoke, canais } = vi.hoisted(() => ({
  invoke: vi.fn(),
  canais: [] as { onmessage: (mensagem: unknown) => void }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  Channel: class {
    onmessage: (mensagem: unknown) => void = () => undefined;
    constructor() {
      canais.push(this);
    }
  },
}));

/** Promessa que o teste resolve ou rejeita quando quiser. */
function promessaControlavel<T>() {
  let resolver!: (valor: T) => void;
  let rejeitar!: (motivo: unknown) => void;
  const promessa = new Promise<T>((resolve, reject) => {
    resolver = resolve;
    rejeitar = reject;
  });
  return { promessa, resolver, rejeitar };
}

/** Clica e espera as atualizações de estado que o clique provoca. */
async function clicar(elemento: HTMLElement) {
  await act(async () => {
    fireEvent.click(elemento);
  });
}

describe("InstalarOllama", () => {
  beforeEach(() => {
    invoke.mockReset();
    canais.length = 0;
  });

  it("mostra o botão de instalar e não instala sozinho", () => {
    render(<InstalarOllama aoConcluir={() => undefined} />);

    expect(
      screen.getByRole("button", { name: "Instalar o Ollama" }),
    ).toBeVisible();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("instala mostrando o progresso do download e avisa quando conclui", async () => {
    const instalacao = promessaControlavel<string>();
    invoke.mockReturnValue(instalacao.promessa);
    const aoConcluir = vi.fn();
    render(<InstalarOllama aoConcluir={aoConcluir} />);

    await clicar(screen.getByRole("button", { name: "Instalar o Ollama" }));

    expect(invoke).toHaveBeenCalledWith("instalar_ollama", {
      canal: canais[0],
    });
    expect(screen.getByText("Preparando…")).toBeVisible();

    act(() =>
      canais[0].onmessage({
        fase: "baixando",
        baixadoEmBytes: 845_000_000,
        totalEmBytes: 1_690_000_000,
      }),
    );
    expect(screen.getByText("Baixando o instalador do Ollama…")).toBeVisible();
    expect(screen.getByRole("progressbar")).toHaveValue(50);
    expect(screen.getByText(/50% \(805,9 MB de 1,6 GB\)/)).toBeVisible();

    act(() =>
      canais[0].onmessage({
        fase: "instalando",
        baixadoEmBytes: 0,
        totalEmBytes: 0,
      }),
    );
    expect(screen.getByText(/Instalando o Ollama/)).toBeVisible();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();

    await act(async () => instalacao.resolver("concluida"));
    expect(aoConcluir).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("permite cancelar enquanto instala", async () => {
    invoke.mockImplementation((comando: string) =>
      comando === "instalar_ollama"
        ? new Promise(() => undefined)
        : Promise.resolve(),
    );
    render(<InstalarOllama aoConcluir={() => undefined} />);
    await clicar(screen.getByRole("button", { name: "Instalar o Ollama" }));

    await clicar(screen.getByRole("button", { name: "Cancelar" }));

    expect(invoke).toHaveBeenCalledWith("cancelar_instalacao_ollama");
  });

  it("mostra a mensagem amigável quando falha e deixa tentar de novo", async () => {
    invoke.mockRejectedValueOnce(
      "Não há espaço livre suficiente para instalar o Ollama.",
    );
    const aoConcluir = vi.fn();
    render(<InstalarOllama aoConcluir={aoConcluir} />);

    await clicar(screen.getByRole("button", { name: "Instalar o Ollama" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não há espaço livre suficiente para instalar o Ollama.",
    );
    expect(aoConcluir).not.toHaveBeenCalled();

    invoke.mockResolvedValueOnce("concluida");
    await clicar(screen.getByRole("button", { name: "Tentar de novo" }));

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(aoConcluir).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("não mostra o erro cru quando a chamada falha de outro jeito", async () => {
    invoke.mockRejectedValue(
      new TypeError("Cannot read properties of undefined"),
    );
    render(<InstalarOllama aoConcluir={() => undefined} />);

    await clicar(screen.getByRole("button", { name: "Instalar o Ollama" }));

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível instalar o Ollama.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });

  it("avisa quando a instalação foi cancelada e deixa tentar de novo", async () => {
    invoke.mockResolvedValue("cancelada");
    const aoConcluir = vi.fn();
    render(<InstalarOllama aoConcluir={aoConcluir} />);

    await clicar(screen.getByRole("button", { name: "Instalar o Ollama" }));

    expect(await screen.findByText("Instalação cancelada.")).toBeVisible();
    expect(aoConcluir).not.toHaveBeenCalled();
    expect(
      screen.getByRole("button", { name: "Tentar de novo" }),
    ).toBeVisible();
  });
});
