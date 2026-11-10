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
import { reiniciarSugestoes, useSugestoes } from "./estado/sugestoes";
import TelaPrincipal from "./TelaPrincipal";
import type {
  Conversa,
  EscopoDeMemoria,
  EstadoOllama,
  Memoria,
  Mensagem,
  Projeto,
  ResultadoDaSugestao,
} from "./tipos";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: class {} }));

const ollama: EstadoOllama = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 1,
  aviso: null,
  mensagem: null,
};

const projeto: Projeto = {
  id: "p-a",
  nome: "Projeto A",
  systemPrompt: "",
  modeloPadrao: "qwen3:8b",
  criadoEm: "",
  atualizadoEm: "",
};

const conversa: Conversa = {
  id: "c1",
  projetoId: "p-a",
  titulo: "Minha conversa",
  modelo: "qwen3:8b",
  temperature: 0.7,
  numCtx: 8192,
  keepAlive: "5m",
  criadoEm: "",
  atualizadoEm: "",
};

function mensagem(
  id: string,
  papel: "user" | "assistant",
  texto: string,
): Mensagem {
  return {
    id,
    conversaId: "c1",
    posicao: id === "m1" ? 1 : 2,
    papel,
    conteudo: texto,
    raciocinio: null,
    estado: "completa",
    criadoEm: "",
    atualizadoEm: "",
  };
}

interface Fake {
  mensagens: Mensagem[];
  memorias: Memoria[];
  /** O que `sugerir_memorias` devolve (ou rejeita). */
  sugerir: () => Promise<ResultadoDaSugestao>;
  falhaAoCriar?: string;
}

function simular(
  sugerir: Fake["sugerir"],
  { comMensagens = true }: { comMensagens?: boolean } = {},
): Fake {
  const fake: Fake = {
    mensagens: comMensagens
      ? [
          mensagem("m1", "user", "Sou a Marina e trabalho com Rust."),
          mensagem("m2", "assistant", "Olá, Marina!"),
        ]
      : [],
    memorias: [],
    sugerir,
  };
  let proximo = 1;
  invoke.mockImplementation(
    async (comando: string, args: Record<string, unknown> = {}) => {
      switch (comando) {
        case "listar_projetos":
          return [projeto];
        case "listar_conversas":
          return [conversa];
        case "listar_mensagens":
          return [...fake.mensagens];
        case "listar_modelos_instalados":
          return ["qwen3:8b"];
        case "detalhes_do_modelo":
          return { janelaDeContexto: null, suportaRaciocinio: true };
        case "listar_memorias_globais":
          return fake.memorias.filter((m) => m.escopo === "global");
        case "listar_memorias_do_projeto":
          return fake.memorias.filter((m) => m.projetoId === args.projetoId);
        case "criar_memoria": {
          if (fake.falhaAoCriar) {
            const mensagemDeErro = fake.falhaAoCriar;
            fake.falhaAoCriar = undefined;
            throw mensagemDeErro;
          }
          const nova: Memoria = {
            id: `mem${proximo++}`,
            escopo: args.escopo as EscopoDeMemoria,
            projetoId: (args.projetoId as string | null) ?? null,
            conteudo: String(args.conteudo),
            criadoEm: "",
            atualizadoEm: "",
          };
          fake.memorias.push(nova);
          return nova;
        }
        case "sugerir_memorias":
          return fake.sugerir();
        case "cancelar_geracao":
          return undefined;
        default:
          throw `comando inesperado: ${comando}`;
      }
    },
  );
  return fake;
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

async function abrirConversa() {
  render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
  const lateral = await screen.findByRole("list", {
    name: "Conversas de Projeto A",
  });
  await clicar(
    await within(lateral).findByRole("button", { name: "Minha conversa" }),
  );
  await screen.findByRole("region", { name: "Conversa" });
}

async function pedirSugestoes() {
  await clicar(botao("Sugerir memórias"));
}

const DUAS = {
  cancelada: false,
  sugestoes: [
    { texto: "O usuário se chama Marina." },
    { texto: "O usuário trabalha com Rust." },
  ],
};

beforeEach(() => {
  invoke.mockReset();
  reiniciarProjetos();
  reiniciarConversas();
  reiniciarMemorias();
  reiniciarSugestoes();
});

describe("Sugestões de memória", () => {
  it("o botão fica na conversa, só com mensagens, e abre o diálogo com as sugestões", async () => {
    simular(() => Promise.resolve(DUAS));
    await abrirConversa();

    await pedirSugestoes();

    const dialogo = await screen.findByRole("dialog", {
      name: "Sugestões de memória",
    });
    expect(
      await within(dialogo).findByText("O usuário se chama Marina."),
    ).toBeVisible();
    expect(
      within(dialogo).getByText("O usuário trabalha com Rust."),
    ).toBeVisible();
    expect(chamadas("sugerir_memorias")[0]?.[1]).toEqual({ conversaId: "c1" });
  });

  it("numa conversa sem mensagens o botão fica desabilitado", async () => {
    simular(() => Promise.resolve(DUAS), { comMensagens: false });
    await abrirConversa();

    expect(botao("Sugerir memórias")).toBeDisabled();
  });

  it("enquanto o modelo responde mostra que está analisando, e nenhuma memória existe ainda", async () => {
    let resolver!: (r: ResultadoDaSugestao) => void;
    const fake = simular(() => new Promise((resolve) => (resolver = resolve)));
    await abrirConversa();

    await pedirSugestoes();

    expect(
      await screen.findByText("Analisando a conversa com o modelo…"),
    ).toBeVisible();
    expect(fake.memorias).toHaveLength(0);
    await act(async () => resolver(DUAS));
    expect(await screen.findByText("O usuário se chama Marina.")).toBeVisible();
    expect(fake.memorias).toHaveLength(0);
  });

  it("aprovar cria exatamente uma memória, no escopo padrão (deste projeto), e a tira da lista", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");

    await clicar(botao(/Aprovar a sugestão “O usuário se chama Marina.”/));

    await waitFor(() =>
      expect(
        screen.queryByText("O usuário se chama Marina."),
      ).not.toBeInTheDocument(),
    );
    expect(chamadas("criar_memoria")).toHaveLength(1);
    expect(chamadas("criar_memoria")[0]?.[1]).toEqual({
      escopo: "projeto",
      projetoId: "p-a",
      conteudo: "O usuário se chama Marina.",
    });
    expect(fake.memorias).toHaveLength(1);
    // A outra sugestão continua na lista, sem decisão.
    expect(screen.getByText("O usuário trabalha com Rust.")).toBeVisible();
  });

  it("aprova no escopo escolhido: global", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário trabalha com Rust.");

    const item = screen
      .getByText("O usuário trabalha com Rust.")
      .closest("li") as HTMLElement;
    await clicar(within(item).getByLabelText("Global"));
    await clicar(within(item).getByRole("button", { name: /Aprovar/ }));

    await waitFor(() => expect(fake.memorias).toHaveLength(1));
    expect(chamadas("criar_memoria")[0]?.[1]).toEqual({
      escopo: "global",
      projetoId: null,
      conteudo: "O usuário trabalha com Rust.",
    });
    expect(fake.memorias[0]?.escopo).toBe("global");
  });

  it("rejeitar tira a sugestão e não cria memória", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");

    await clicar(botao(/Rejeitar a sugestão “O usuário se chama Marina.”/));

    expect(
      screen.queryByText("O usuário se chama Marina."),
    ).not.toBeInTheDocument();
    expect(chamadas("criar_memoria")).toHaveLength(0);
    expect(fake.memorias).toHaveLength(0);
  });

  it("fechar o diálogo sem decidir não cria memória alguma (falha)", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");

    await clicar(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Fechar",
      }),
    );

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(chamadas("criar_memoria")).toHaveLength(0);
    expect(fake.memorias).toHaveLength(0);
  });

  it("Esc também fecha o diálogo sem criar nada (falha)", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape" });
    });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(fake.memorias).toHaveLength(0);
  });

  it("fechar enquanto o modelo ainda responde cancela a chamada e descarta a resposta tardia (falha)", async () => {
    let resolver!: (r: ResultadoDaSugestao) => void;
    const fake = simular(() => new Promise((resolve) => (resolver = resolve)));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("Analisando a conversa com o modelo…");

    await clicar(botao("Cancelar"));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(chamadas("cancelar_geracao")).toHaveLength(1);
    // A resposta chega depois de fechar: nada aparece e nada é criado.
    await act(async () => resolver(DUAS));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useSugestoes.getState().sugestoes).toEqual([]);
    expect(fake.memorias).toHaveLength(0);
  });

  it("resposta malformada mostra mensagem amigável, nenhuma memória, e dá para tentar de novo (falha)", async () => {
    let tentativa = 0;
    const fake = simular(() => {
      tentativa += 1;
      return tentativa === 1
        ? Promise.reject(
            "O modelo não devolveu as sugestões num formato que o Memoteca entenda. Nenhuma memória foi criada. Tente de novo ou use outro modelo.",
          )
        : Promise.resolve(DUAS);
    });
    await abrirConversa();

    await pedirSugestoes();

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Nenhuma memória foi criada");
    expect(alerta).not.toHaveTextContent("JSON");
    expect(fake.memorias).toHaveLength(0);

    await clicar(botao("Tentar de novo"));
    expect(await screen.findByText("O usuário se chama Marina.")).toBeVisible();
    expect(chamadas("sugerir_memorias")).toHaveLength(2);
  });

  it("erro que não é texto vira a mensagem padrão, nunca o erro cru", async () => {
    simular(() => Promise.reject(new TypeError("Cannot read properties")));
    await abrirConversa();

    await pedirSugestoes();

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível concluir.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });

  it("lista vazia mostra que o modelo não achou nada, sem criar memória", async () => {
    const fake = simular(() =>
      Promise.resolve({ cancelada: false, sugestoes: [] }),
    );
    await abrirConversa();

    await pedirSugestoes();

    expect(
      await screen.findByText(/não encontrou nada que valha a pena guardar/),
    ).toBeVisible();
    expect(fake.memorias).toHaveLength(0);
  });

  it("depois de decidir todas, diz quantas memórias foram guardadas", async () => {
    simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");

    await clicar(botao(/Aprovar a sugestão “O usuário se chama Marina.”/));
    await clicar(botao(/Rejeitar a sugestão “O usuário trabalha com Rust.”/));

    expect(
      await screen.findByText("Pronto: 1 memória guardada."),
    ).toBeVisible();
  });

  it("se a aprovação falha, a sugestão continua na lista com a mensagem amigável", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");
    fake.falhaAoCriar =
      "Uma memória pode ter no máximo 2000 caracteres. Resuma o texto.";

    await clicar(botao(/Aprovar a sugestão “O usuário se chama Marina.”/));

    const item = screen
      .getByText("O usuário se chama Marina.")
      .closest("li") as HTMLElement;
    expect(await within(item).findByRole("alert")).toHaveTextContent(
      "no máximo 2000 caracteres",
    );
    expect(fake.memorias).toHaveLength(0);
  });

  it("a chamada extra não mexe nas mensagens da conversa", async () => {
    const fake = simular(() => Promise.resolve(DUAS));
    await abrirConversa();
    const antes = [...fake.mensagens];

    await pedirSugestoes();
    await screen.findByText("O usuário se chama Marina.");
    await clicar(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Fechar",
      }),
    );

    expect(fake.mensagens).toEqual(antes);
    expect(chamadas("enviar_mensagem")).toHaveLength(0);
    expect(chamadas("regenerar_resposta")).toHaveLength(0);
    expect(screen.getByText("Olá, Marina!")).toBeVisible();
  });
});
