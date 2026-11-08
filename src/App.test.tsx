import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
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
    invoke.mockRejectedValue(
      "Não foi possível abrir os dados do Memoteca. Seus dados não foram apagados.",
    );
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent(
      "Não foi possível abrir os dados do Memoteca. Seus dados não foram apagados.",
    );
    expect(screen.queryByText("Em construção.")).not.toBeInTheDocument();
  });

  it("mostra uma mensagem amigável, e não o erro cru, quando a consulta falha de outro jeito", async () => {
    invoke.mockRejectedValue(
      new TypeError("Cannot read properties of undefined"),
    );
    render(<App />);

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent(
      "Não foi possível verificar os dados do Memoteca.",
    );
    expect(alerta).not.toHaveTextContent("Cannot read");
  });
});
