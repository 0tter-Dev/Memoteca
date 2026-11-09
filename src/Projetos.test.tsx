import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { reiniciarProjetos } from "./estado/projetos";
import TelaPrincipal from "./TelaPrincipal";
import type { EstadoOllama, Projeto, ResumoExclusaoProjeto } from "./tipos";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const ollama: EstadoOllama = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 2,
  aviso: null,
  mensagem: null,
};

function projeto(parcial: Partial<Projeto> & { nome: string }): Projeto {
  return {
    id: parcial.id ?? `id-${parcial.nome}`,
    systemPrompt: "",
    modeloPadrao: null,
    criadoEm: "2026-11-08T12:00:00.000Z",
    atualizadoEm: "2026-11-08T12:00:00.000Z",
    ...parcial,
  };
}

interface Opcoes {
  instalados?: string[] | "falha";
  resumo?: ResumoExclusaoProjeto;
  falhaAoCriar?: string;
  falhaAoListar?: string;
}

/** Um backend de projetos em memória, com a mesma ordem alfabética do real. */
function simularBackend(inicial: Projeto[] = [], opcoes: Opcoes = {}) {
  let projetos = [...inicial];
  let proximoId = 1;
  const ordenados = () =>
    [...projetos].sort((a, b) =>
      a.nome.localeCompare(b.nome, "pt", { sensitivity: "base" }),
    );

  invoke.mockImplementation((comando: string, args?: Record<string, never>) => {
    switch (comando) {
      case "listar_projetos":
        return opcoes.falhaAoListar
          ? Promise.reject(opcoes.falhaAoListar)
          : Promise.resolve(ordenados());
      case "listar_modelos_instalados":
        return opcoes.instalados === "falha"
          ? Promise.reject("Não foi possível listar os modelos agora.")
          : Promise.resolve(opcoes.instalados ?? ["llama3.2:3b", "qwen3:8b"]);
      case "criar_projeto": {
        if (opcoes.falhaAoCriar) {
          return Promise.reject(opcoes.falhaAoCriar);
        }
        const dados = (args as unknown as { dados: Omit<Projeto, "id"> }).dados;
        const novo = projeto({
          ...dados,
          nome: dados.nome.trim(),
          id: `novo-${proximoId++}`,
        });
        projetos.push(novo);
        return Promise.resolve(novo);
      }
      case "atualizar_projeto": {
        const { id, dados } = args as unknown as {
          id: string;
          dados: Omit<Projeto, "id">;
        };
        projetos = projetos.map((p) => (p.id === id ? { ...p, ...dados } : p));
        return Promise.resolve(projetos.find((p) => p.id === id));
      }
      case "resumo_exclusao_projeto":
        return Promise.resolve(
          opcoes.resumo ?? { conversas: 0, mensagens: 0, memorias: 0 },
        );
      case "excluir_projeto": {
        const { id } = args as unknown as { id: string };
        projetos = projetos.filter((p) => p.id !== id);
        return Promise.resolve();
      }
      default:
        return Promise.reject(`comando inesperado: ${comando}`);
    }
  });
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

function renderizar() {
  render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
}

describe("Projetos: lista e detalhe", () => {
  beforeEach(() => {
    invoke.mockReset();
    reiniciarProjetos();
  });

  it("sem projetos, convida a criar o primeiro", async () => {
    simularBackend();
    renderizar();

    expect(await screen.findByText("Nenhum projeto ainda")).toBeVisible();
    expect(botao("Criar o primeiro projeto")).toBeVisible();
  });

  it("lista os projetos em ordem alfabética na lateral e abre o primeiro", async () => {
    simularBackend([
      projeto({ nome: "Receitas", systemPrompt: "Seja gentil." }),
      projeto({ nome: "abacaxi" }),
      projeto({ nome: "Estudos" }),
    ]);
    renderizar();

    const lista = await screen.findByRole("navigation", {
      name: "Lista de projetos",
    });
    const nomes = within(lista)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(nomes).toEqual(["abacaxi", "Estudos", "Receitas"]);
    expect(
      within(lista).getByRole("button", { name: "abacaxi" }),
    ).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("heading", { name: "abacaxi" })).toBeVisible();
  });

  it("mostra as instruções e o modelo padrão do projeto escolhido", async () => {
    simularBackend([
      projeto({
        nome: "Receitas",
        systemPrompt: "Seja gentil.\nResponda em português.",
        modeloPadrao: "qwen3:8b",
      }),
      projeto({ nome: "Sem nada" }),
    ]);
    renderizar();
    await screen.findByRole("heading", { name: "Receitas" });

    expect(screen.getByText(/Seja gentil\./)).toBeVisible();
    expect(screen.getByText("qwen3:8b")).toBeVisible();

    await clicar(botao("Sem nada"));

    expect(screen.getByText("Nenhuma instrução definida.")).toBeVisible();
    expect(
      screen.getByText(/vale o modelo da primeira conversa criada/),
    ).toBeVisible();
  });

  it("mostra o estado do Ollama na lateral", async () => {
    simularBackend([projeto({ nome: "A" })]);
    renderizar();

    expect(await screen.findByText("Ollama 0.32.9 em execução.")).toBeVisible();
  });

  it("mostra uma mensagem amigável quando a lista não carrega", async () => {
    simularBackend([], {
      falhaAoListar: "Não foi possível abrir os dados do Memoteca.",
    });
    renderizar();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível abrir os dados do Memoteca.",
    );
  });

  it("não existe caminho na interface para criar conversa fora de um projeto", async () => {
    simularBackend([projeto({ nome: "A" })]);
    renderizar();
    await screen.findByRole("heading", { name: "A" });

    // Nem com projetos, nem no formulário, nem sem projeto algum.
    expect(screen.queryByText(/nova conversa/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /conversa/i }),
    ).not.toBeInTheDocument();
    await clicar(botao("Novo projeto"));
    expect(screen.queryByText(/nova conversa/i)).not.toBeInTheDocument();
  });
});

describe("Projetos: criar e editar", () => {
  beforeEach(() => {
    invoke.mockReset();
    reiniciarProjetos();
  });

  it("cria um projeto com nome, instruções e modelo padrão", async () => {
    simularBackend();
    renderizar();
    await clicar(
      await screen.findByRole("button", { name: "Criar o primeiro projeto" }),
    );

    digitar("Nome do projeto", "Estudos de Rust");
    digitar(/Instruções do projeto/, "Responda em português.");
    await screen.findByRole("option", { name: "qwen3:8b" });
    fireEvent.change(screen.getByLabelText(/Modelo padrão/), {
      target: { value: "qwen3:8b" },
    });
    await clicar(botao("Criar projeto"));

    expect(chamadas("criar_projeto")[0][1]).toEqual({
      dados: {
        nome: "Estudos de Rust",
        systemPrompt: "Responda em português.",
        modeloPadrao: "qwen3:8b",
      },
    });
    // Depois de salvar, o projeto aparece na lateral e fica aberto.
    expect(
      await screen.findByRole("heading", { name: "Estudos de Rust" }),
    ).toBeVisible();
    expect(screen.getByText("Responda em português.")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Estudos de Rust" }),
    ).toHaveAttribute("aria-current", "true");
  });

  it("sem modelo escolhido, o modelo padrão vai como vazio", async () => {
    simularBackend();
    renderizar();
    await clicar(
      await screen.findByRole("button", { name: "Criar o primeiro projeto" }),
    );

    digitar("Nome do projeto", "Sem modelo");
    await clicar(botao("Criar projeto"));

    expect(chamadas("criar_projeto")[0][1]).toEqual({
      dados: { nome: "Sem modelo", systemPrompt: "", modeloPadrao: null },
    });
  });

  it("o nome é obrigatório e a recusa não chega ao backend", async () => {
    simularBackend();
    renderizar();
    await clicar(
      await screen.findByRole("button", { name: "Criar o primeiro projeto" }),
    );

    digitar("Nome do projeto", "   ");
    await clicar(botao("Criar projeto"));

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Dê um nome ao projeto.",
    );
    expect(chamadas("criar_projeto")).toHaveLength(0);
  });

  it("mostra a mensagem amigável que o backend devolver ao criar", async () => {
    simularBackend([], {
      falhaAoCriar: "O nome do projeto pode ter no máximo 100 caracteres.",
    });
    renderizar();
    await clicar(
      await screen.findByRole("button", { name: "Criar o primeiro projeto" }),
    );

    digitar("Nome do projeto", "Qualquer");
    await clicar(botao("Criar projeto"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "O nome do projeto pode ter no máximo 100 caracteres.",
    );
    // O formulário continua aberto, com o que foi digitado.
    expect(screen.getByLabelText("Nome do projeto")).toHaveValue("Qualquer");
  });

  it("cancelar o formulário volta ao projeto sem salvar nada", async () => {
    simularBackend([projeto({ nome: "Fica" })]);
    renderizar();
    await screen.findByRole("heading", { name: "Fica" });

    await clicar(botao("Novo projeto"));
    digitar("Nome do projeto", "Descartado");
    await clicar(botao("Cancelar"));

    expect(chamadas("criar_projeto")).toHaveLength(0);
    expect(screen.getByRole("heading", { name: "Fica" })).toBeVisible();
  });

  it("edita um projeto e envia o id com os dados novos", async () => {
    simularBackend([
      projeto({
        nome: "Velho",
        systemPrompt: "Antigo.",
        modeloPadrao: "llama3.2:3b",
      }),
    ]);
    renderizar();
    await clicar(await screen.findByRole("button", { name: "Editar" }));

    expect(screen.getByLabelText("Nome do projeto")).toHaveValue("Velho");
    expect(screen.getByLabelText(/Instruções do projeto/)).toHaveValue(
      "Antigo.",
    );
    digitar("Nome do projeto", "Novo");
    digitar(/Instruções do projeto/, "Atual.");
    await clicar(botao("Salvar alterações"));

    expect(chamadas("atualizar_projeto")[0][1]).toEqual({
      id: "id-Velho",
      dados: {
        nome: "Novo",
        systemPrompt: "Atual.",
        modeloPadrao: "llama3.2:3b",
      },
    });
    expect(await screen.findByRole("heading", { name: "Novo" })).toBeVisible();
  });

  it("o modelo padrão salvo continua escolhível mesmo se não estiver mais instalado", async () => {
    simularBackend(
      [projeto({ nome: "P", modeloPadrao: "modelo-removido:1b" })],
      {
        instalados: ["qwen3:8b"],
      },
    );
    renderizar();
    await clicar(await screen.findByRole("button", { name: "Editar" }));

    const seletor = await screen.findByRole("option", { name: "qwen3:8b" });
    expect(seletor).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "modelo-removido:1b" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/Modelo padrão/)).toHaveValue(
      "modelo-removido:1b",
    );
  });

  it("escolher a opção sem modelo volta o modelo padrão a vazio", async () => {
    simularBackend([projeto({ nome: "P", modeloPadrao: "qwen3:8b" })]);
    renderizar();
    await clicar(await screen.findByRole("button", { name: "Editar" }));
    await screen.findByRole("option", { name: "llama3.2:3b" });

    fireEvent.change(screen.getByLabelText(/Modelo padrão/), {
      target: { value: "" },
    });
    await clicar(botao("Salvar alterações"));

    expect(chamadas("atualizar_projeto")[0][1]).toMatchObject({
      dados: { modeloPadrao: null },
    });
  });

  it("se a lista de modelos não carregar, o formulário continua funcionando", async () => {
    simularBackend([], { instalados: "falha" });
    renderizar();
    await clicar(
      await screen.findByRole("button", { name: "Criar o primeiro projeto" }),
    );

    digitar("Nome do projeto", "Sem lista");
    await clicar(botao("Criar projeto"));

    expect(chamadas("criar_projeto")).toHaveLength(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Projetos: excluir (com confirmação)", () => {
  beforeEach(() => {
    invoke.mockReset();
    reiniciarProjetos();
  });

  async function abrirConfirmacao() {
    await clicar(await screen.findByRole("button", { name: "Editar" }));
    await clicar(botao("Excluir projeto"));
    return screen.findByRole("alertdialog");
  }

  it("pede confirmação e mostra o que será apagado, sem apagar ainda", async () => {
    simularBackend([projeto({ nome: "Antigo" })], {
      resumo: { conversas: 2, mensagens: 5, memorias: 1 },
    });
    renderizar();

    const dialogo = await abrirConfirmacao();

    expect(
      within(dialogo).getByText(/Excluir o projeto "Antigo"\?/),
    ).toBeVisible();
    expect(
      await within(dialogo).findByText(/2 conversas, 5 mensagens e 1 memória/),
    ).toBeVisible();
    expect(
      within(dialogo).getByText(/As memórias globais não são afetadas/),
    ).toBeVisible();
    expect(within(dialogo).getByText(/não pode ser desfeita/)).toBeVisible();
    expect(chamadas("excluir_projeto")).toHaveLength(0);
  });

  it("cancelar não apaga nada", async () => {
    simularBackend([projeto({ nome: "Fica" })]);
    renderizar();
    const dialogo = await abrirConfirmacao();

    await clicar(within(dialogo).getByRole("button", { name: "Cancelar" }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(chamadas("excluir_projeto")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Fica" })).toBeVisible();
  });

  it("confirmar apaga o projeto e volta ao estado sem projetos", async () => {
    simularBackend([projeto({ nome: "Sai" })], {
      resumo: { conversas: 0, mensagens: 0, memorias: 0 },
    });
    renderizar();
    const dialogo = await abrirConfirmacao();
    expect(
      await within(dialogo).findByText(/ainda não tem conversas nem memórias/),
    ).toBeVisible();

    await clicar(
      within(dialogo).getByRole("button", { name: "Excluir projeto" }),
    );

    expect(chamadas("excluir_projeto")[0][1]).toEqual({ id: "id-Sai" });
    expect(await screen.findByText("Nenhum projeto ainda")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Sai" }),
    ).not.toBeInTheDocument();
  });

  it("só pode excluir depois de saber o que será apagado", async () => {
    let liberar!: (resumo: ResumoExclusaoProjeto) => void;
    simularBackend([projeto({ nome: "P" })]);
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation(
      (comando: string, args?: Record<string, never>) =>
        comando === "resumo_exclusao_projeto"
          ? new Promise<ResumoExclusaoProjeto>((resolve) => {
              liberar = resolve;
            })
          : original(comando, args),
    );
    renderizar();
    const dialogo = await abrirConfirmacao();

    expect(
      within(dialogo).getByRole("button", { name: "Excluir projeto" }),
    ).toBeDisabled();

    await act(async () => liberar({ conversas: 1, mensagens: 2, memorias: 0 }));

    expect(
      within(dialogo).getByRole("button", { name: "Excluir projeto" }),
    ).toBeEnabled();
  });

  it("se a exclusão falhar, mostra a mensagem amigável e mantém o projeto", async () => {
    simularBackend([projeto({ nome: "Fica" })]);
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation(
      (comando: string, args?: Record<string, never>) =>
        comando === "excluir_projeto"
          ? Promise.reject(
              "Não encontramos este item. Ele pode ter sido excluído.",
            )
          : original(comando, args),
    );
    renderizar();
    const dialogo = await abrirConfirmacao();
    await within(dialogo).findByText(/não pode ser desfeita/);

    await clicar(
      within(dialogo).getByRole("button", { name: "Excluir projeto" }),
    );

    expect(await within(dialogo).findByRole("alert")).toHaveTextContent(
      "Não encontramos este item.",
    );
    // Com o diálogo aberto, o resto da tela fica oculto para leitores de tela.
    expect(
      screen.getByRole("button", { name: "Fica", hidden: true }),
    ).toBeInTheDocument();
  });
});
