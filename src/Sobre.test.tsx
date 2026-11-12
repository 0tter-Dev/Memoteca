import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AVISO_DE_INDEPENDENCIA, REPOSITORIO } from "./avisoDeIndependencia";
import { reiniciarConversas } from "./estado/conversas";
import { reiniciarMemorias } from "./estado/memorias";
import { reiniciarProjetos } from "./estado/projetos";
import { reiniciarSugestoes } from "./estado/sugestoes";
import TelaPrincipal from "./TelaPrincipal";
import type { EstadoOllama } from "./tipos";

const invoke = vi.hoisted(() => vi.fn());
const getVersion = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: class {} }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion }));

const ollama: EstadoOllama = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 1,
  aviso: null,
  mensagem: null,
};

describe("tela Sobre (E16)", () => {
  beforeEach(() => {
    invoke.mockReset();
    getVersion.mockReset();
    reiniciarProjetos();
    reiniciarConversas();
    reiniciarMemorias();
    reiniciarSugestoes();
    invoke.mockImplementation(async (comando: string) => {
      if (comando === "listar_projetos") return [];
      return [];
    });
    getVersion.mockResolvedValue("0.1.0");
  });

  it("abre pela lateral e mostra o aviso de independência, a versão, a licença e o repositório", async () => {
    render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
    fireEvent.click(await screen.findByRole("button", { name: /Sobre/ }));

    const tela = await screen.findByRole("article", {
      name: "Sobre o Memoteca",
    });
    expect(tela).toHaveTextContent(AVISO_DE_INDEPENDENCIA);
    expect(tela).toHaveTextContent("MIT, Memoteca contributors");
    expect(tela).toHaveTextContent(REPOSITORIO);
    await waitFor(() => expect(tela).toHaveTextContent("0.1.0"));
  });

  it("não sugere que o app é do Ollama: o aviso nega a afiliação e o nome do app é Memoteca", async () => {
    render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
    fireEvent.click(await screen.findByRole("button", { name: /Sobre/ }));

    const tela = await screen.findByRole("article", {
      name: "Sobre o Memoteca",
    });
    expect(tela).toHaveTextContent("Este é um projeto independente");
    expect(tela).toHaveTextContent("não é afiliado, patrocinado, endossado");
    expect(tela).toHaveTextContent("não redistribui o Ollama");
    expect(tela.textContent).not.toMatch(/OllamaUI/i);
    expect(tela.querySelector("img, svg")).toBeNull();
  });

  it("continua útil quando a versão não pode ser lida", async () => {
    getVersion.mockRejectedValue(new Error("falhou"));
    render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
    fireEvent.click(await screen.findByRole("button", { name: /Sobre/ }));

    const tela = await screen.findByRole("article", {
      name: "Sobre o Memoteca",
    });
    expect(tela).toHaveTextContent(AVISO_DE_INDEPENDENCIA);
    expect(tela.textContent).not.toMatch(/falhou|Error/);
  });
});
