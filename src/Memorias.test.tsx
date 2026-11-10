import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { reiniciarConversas } from "./estado/conversas";
import { reiniciarMemorias } from "./estado/memorias";
import { reiniciarProjetos } from "./estado/projetos";
import TelaPrincipal from "./TelaPrincipal";
import type { EscopoDeMemoria, EstadoOllama, Memoria, Projeto } from "./tipos";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: class {} }));

const ollama: EstadoOllama = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 2,
  aviso: null,
  mensagem: null,
};

const DATA = "2026-11-09T12:00:00.000Z";

function projeto(nome: string): Projeto {
  return {
    id: `p-${nome}`,
    nome,
    systemPrompt: "",
    modeloPadrao: null,
    criadoEm: DATA,
    atualizadoEm: DATA,
  };
}

function memoria(
  id: string,
  escopo: EscopoDeMemoria,
  conteudo: string,
  projetoId: string | null = null,
): Memoria {
  return {
    id,
    escopo,
    projetoId,
    conteudo,
    criadoEm: DATA,
    atualizadoEm: DATA,
  };
}

interface Backend {
  memorias: Memoria[];
  /** Falha (mensagem amigável) que o próximo comando de memória deve devolver. */
  falha?: string;
}

/** Backend de memórias em memória, que valida como o real (texto em branco é recusado). */
function simular(projetos: Projeto[], memoriasIniciais: Memoria[] = []) {
  const estado: Backend = { memorias: [...memoriasIniciais] };
  let proximo = 1;
  invoke.mockImplementation((comando: string, args?: Record<string, never>) => {
    const entrada = (args ?? {}) as Record<string, string | null>;
    if (
      estado.falha &&
      ["criar_memoria", "atualizar_memoria", "excluir_memoria"].includes(
        comando,
      )
    ) {
      const mensagem = estado.falha;
      estado.falha = undefined;
      return Promise.reject(mensagem);
    }
    switch (comando) {
      case "listar_projetos":
        return Promise.resolve(projetos);
      case "listar_conversas":
        return Promise.resolve([]);
      case "listar_memorias_globais":
        return Promise.resolve(
          estado.memorias.filter((m) => m.escopo === "global"),
        );
      case "listar_memorias_do_projeto":
        return Promise.resolve(
          estado.memorias.filter((m) => m.projetoId === entrada.projetoId),
        );
      case "criar_memoria": {
        const texto = String(entrada.conteudo).trim();
        if (texto === "") {
          return Promise.reject("Escreva o que o modelo deve lembrar.");
        }
        const nova = memoria(
          `m${proximo++}`,
          entrada.escopo as EscopoDeMemoria,
          texto,
          entrada.projetoId ?? null,
        );
        estado.memorias.push(nova);
        return Promise.resolve(nova);
      }
      case "atualizar_memoria": {
        const alvo = estado.memorias.find((m) => m.id === entrada.id);
        if (alvo) {
          alvo.conteudo = String(entrada.conteudo).trim();
        }
        return Promise.resolve(alvo);
      }
      case "excluir_memoria":
        estado.memorias = estado.memorias.filter((m) => m.id !== entrada.id);
        return Promise.resolve();
      default:
        return Promise.reject(`comando inesperado: ${comando}`);
    }
  });
  return estado;
}

function chamadas(comando: string) {
  return invoke.mock.calls.filter(([c]) => c === comando);
}

async function clicar(elemento: HTMLElement) {
  await act(async () => {
    fireEvent.click(elemento);
  });
}

function botao(nome: string | RegExp) {
  return screen.getByRole("button", { name: nome });
}

function digitar(rotulo: string | RegExp, valor: string) {
  fireEvent.change(screen.getByLabelText(rotulo), { target: { value: valor } });
}

async function abrir() {
  render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
  await screen.findByRole("heading", { level: 2, name: "Rust" });
}

async function abrirMemoriasDoProjeto() {
  await clicar(botao("Gerenciar memórias"));
  await screen.findByRole("tablist", { name: "Escopo das memórias" });
}

beforeEach(() => {
  invoke.mockReset();
  reiniciarProjetos();
  reiniciarConversas();
  reiniciarMemorias();
});

describe("Memórias", () => {
  it("o projeto tem um atalho para as memórias dele, na aba do projeto", async () => {
    simular(
      [projeto("Rust")],
      [
        memoria("m1", "projeto", "Usa Rust 2024", "p-Rust"),
        memoria("m2", "global", "Mora em Recife"),
      ],
    );
    await abrir();

    await abrirMemoriasDoProjeto();

    expect(screen.getByRole("tab", { name: "Deste projeto" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByText("Usa Rust 2024")).toBeVisible();
    expect(screen.queryByText("Mora em Recife")).not.toBeInTheDocument();
  });

  it("a memória global fica na lateral e mostra só as globais, de qualquer projeto", async () => {
    simular(
      [projeto("Rust")],
      [
        memoria("m1", "projeto", "Só deste projeto", "p-Rust"),
        memoria("m2", "global", "Vale para tudo"),
      ],
    );
    await abrir();

    await clicar(botao("◇ Memória global"));

    expect(await screen.findByText("Vale para tudo")).toBeVisible();
    expect(screen.queryByText("Só deste projeto")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Globais" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("alterna entre as abas Deste projeto e Globais", async () => {
    simular(
      [projeto("Rust")],
      [
        memoria("m1", "projeto", "Do projeto", "p-Rust"),
        memoria("m2", "global", "Global"),
      ],
    );
    await abrir();
    await abrirMemoriasDoProjeto();

    await clicar(screen.getByRole("tab", { name: "Globais" }));
    expect(screen.getByText("Global")).toBeVisible();
    expect(screen.queryByText("Do projeto")).not.toBeInTheDocument();

    await clicar(screen.getByRole("tab", { name: "Deste projeto" }));
    expect(screen.getByText("Do projeto")).toBeVisible();
  });

  it("clicar em Memória global com a tela de memórias já aberta troca para a aba Globais", async () => {
    simular(
      [projeto("Rust")],
      [
        memoria("m1", "projeto", "Do projeto", "p-Rust"),
        memoria("m2", "global", "Global"),
      ],
    );
    await abrir();
    await abrirMemoriasDoProjeto();
    expect(screen.getByText("Do projeto")).toBeVisible();

    await clicar(botao("◇ Memória global"));

    expect(await screen.findByText("Global")).toBeVisible();
    expect(screen.queryByText("Do projeto")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Globais" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("cria uma memória de projeto e uma global, cada uma no seu escopo", async () => {
    const backend = simular([projeto("Rust")]);
    await abrir();
    await abrirMemoriasDoProjeto();

    digitar("Nova memória", "  Prefere exemplos curtos  ");
    await clicar(botao("Adicionar memória"));
    expect(await screen.findByText("Prefere exemplos curtos")).toBeVisible();
    expect(screen.getByLabelText("Nova memória")).toHaveValue("");

    await clicar(screen.getByRole("tab", { name: "Globais" }));
    digitar("Nova memória", "Fala português");
    await clicar(botao("Adicionar memória"));
    expect(await screen.findByText("Fala português")).toBeVisible();

    expect(chamadas("criar_memoria").map(([, args]) => args)).toEqual([
      {
        escopo: "projeto",
        projetoId: "p-Rust",
        conteudo: "  Prefere exemplos curtos  ",
      },
      { escopo: "global", projetoId: null, conteudo: "Fala português" },
    ]);
    expect(backend.memorias.map((m) => [m.escopo, m.projetoId])).toEqual([
      ["projeto", "p-Rust"],
      ["global", null],
    ]);
  });

  it("não deixa adicionar texto em branco", async () => {
    simular([projeto("Rust")]);
    await abrir();
    await abrirMemoriasDoProjeto();

    digitar("Nova memória", "   ");

    expect(botao("Adicionar memória")).toBeDisabled();
    expect(chamadas("criar_memoria")).toHaveLength(0);
  });

  it("mostra a mensagem amigável quando criar falha, e o texto digitado fica", async () => {
    const backend = simular([projeto("Rust")]);
    await abrir();
    await abrirMemoriasDoProjeto();
    backend.falha =
      "Uma memória pode ter no máximo 2000 caracteres. Resuma o texto.";

    digitar("Nova memória", "Algo");
    await clicar(botao("Adicionar memória"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Uma memória pode ter no máximo 2000 caracteres",
    );
    expect(screen.getByLabelText("Nova memória")).toHaveValue("Algo");
  });

  it("mostra o contador de caracteres e avisa quando passa do limite", async () => {
    simular([projeto("Rust")]);
    await abrir();
    await abrirMemoriasDoProjeto();

    digitar("Nova memória", "abc");
    expect(screen.getByText("3 de 2000 caracteres")).toBeVisible();

    digitar("Nova memória", "a".repeat(2001));
    expect(screen.getByText("2001 de 2000 caracteres")).toHaveClass(
      "text-red-700",
    );
  });

  it("edita uma memória e a lista mostra o texto novo", async () => {
    const backend = simular(
      [projeto("Rust")],
      [memoria("m1", "projeto", "Gosta de café", "p-Rust")],
    );
    await abrir();
    await abrirMemoriasDoProjeto();

    await clicar(botao(/Editar a memória “Gosta de café”/));
    digitar("Texto da memória", "Gosta de chá");
    await clicar(botao("Salvar"));

    expect(await screen.findByText("Gosta de chá")).toBeVisible();
    expect(screen.queryByText("Gosta de café")).not.toBeInTheDocument();
    expect(backend.memorias[0]?.conteudo).toBe("Gosta de chá");
    expect(chamadas("atualizar_memoria")[0]?.[1]).toEqual({
      id: "m1",
      conteudo: "Gosta de chá",
    });
  });

  it("cancelar a edição não salva nada e volta o texto original", async () => {
    simular(
      [projeto("Rust")],
      [memoria("m1", "projeto", "Original", "p-Rust")],
    );
    await abrir();
    await abrirMemoriasDoProjeto();

    await clicar(botao(/Editar a memória/));
    digitar("Texto da memória", "Mudei de ideia");
    await clicar(botao("Cancelar"));

    expect(screen.getByText("Original")).toBeVisible();
    expect(chamadas("atualizar_memoria")).toHaveLength(0);
  });

  it("remove a memória em silêncio: some da lista e a tela não mostra aviso de contexto", async () => {
    const backend = simular(
      [projeto("Rust")],
      [
        memoria("m1", "projeto", "Apagar esta", "p-Rust"),
        memoria("m2", "projeto", "Manter esta", "p-Rust"),
      ],
    );
    await abrir();
    await abrirMemoriasDoProjeto();

    await clicar(botao(/Remover a memória “Apagar esta”/));

    await waitFor(() =>
      expect(screen.queryByText("Apagar esta")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Manter esta")).toBeVisible();
    expect(backend.memorias.map((m) => m.id)).toEqual(["m2"]);
    // Silenciosa: sem diálogo, sem alerta e sem nenhum texto de "contexto mudou".
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    // O único aviso de status na tela é o do Ollama, na lateral.
    for (const status of screen.queryAllByRole("status")) {
      expect(status).toHaveTextContent("Ollama");
    }
    expect(document.body.textContent?.toLowerCase()).not.toContain(
      "contexto mudou",
    );
    expect(document.body.textContent?.toLowerCase()).not.toContain(
      "contexto foi alterado",
    );
  });

  it("as memórias carregam do banco ao abrir a tela, então persistem entre sessões", async () => {
    simular(
      [projeto("Rust")],
      [memoria("m1", "global", "Guardada antes de fechar o app")],
    );
    await abrir();

    await clicar(botao("◇ Memória global"));

    expect(
      await screen.findByText("Guardada antes de fechar o app"),
    ).toBeVisible();
    expect(chamadas("listar_memorias_globais").length).toBeGreaterThan(0);
  });

  it("sem nenhum projeto, as memórias globais continuam acessíveis", async () => {
    simular([], [memoria("m1", "global", "Vale sempre")]);
    render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
    await screen.findByText("Nenhum projeto ainda");

    await clicar(botao("◇ Memória global"));

    expect(await screen.findByText("Vale sempre")).toBeVisible();
    expect(
      screen.queryByRole("tab", { name: "Deste projeto" }),
    ).not.toBeInTheDocument();
  });

  it("Voltar leva de volta ao projeto, e escolher um projeto na lateral também sai das memórias", async () => {
    simular([projeto("Rust")]);
    await abrir();
    await abrirMemoriasDoProjeto();

    await clicar(botao("Voltar"));
    expect(
      await screen.findByRole("heading", { level: 2, name: "Rust" }),
    ).toBeVisible();

    await clicar(botao("◇ Memória global"));
    await screen.findByRole("tablist", { name: "Escopo das memórias" });
    const lista = screen.getByRole("navigation", { name: "Lista de projetos" });
    await clicar(within(lista).getByRole("button", { name: "Rust" }));
    expect(
      screen.queryByRole("tablist", { name: "Escopo das memórias" }),
    ).not.toBeInTheDocument();
  });
});
