import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { reiniciarConversas } from "./estado/conversas";
import { reiniciarProjetos } from "./estado/projetos";
import TelaPrincipal from "./TelaPrincipal";
import type {
  Conversa,
  EstadoOllama,
  EventoDeGeracao,
  Mensagem,
  Projeto,
  ResultadoDaGeracao,
} from "./tipos";

const { invoke, canais } = vi.hoisted(() => ({
  invoke: vi.fn(),
  canais: [] as { onmessage: (evento: unknown) => void }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  Channel: class {
    onmessage: (evento: unknown) => void = () => undefined;
    constructor() {
      canais.push(this);
    }
  },
}));

const ollama: EstadoOllama = {
  estado: "em_execucao",
  versao: "0.32.9",
  origem: "externo",
  modelosInstalados: 2,
  aviso: null,
  mensagem: null,
};

const projetoA: Projeto = {
  id: "p-a",
  nome: "Projeto A",
  systemPrompt: "",
  modeloPadrao: "qwen3:8b",
  criadoEm: "",
  atualizadoEm: "",
};

function conversa(parcial: Partial<Conversa> & { id: string }): Conversa {
  return {
    projetoId: "p-a",
    titulo: "Nova conversa",
    modelo: "qwen3:8b",
    temperature: 0.7,
    numCtx: 8192,
    keepAlive: "5m",
    criadoEm: "",
    atualizadoEm: "",
    ...parcial,
  };
}

function mensagem(parcial: Partial<Mensagem> & { id: string }): Mensagem {
  return {
    conversaId: "c1",
    posicao: 1,
    papel: "user",
    conteudo: "",
    raciocinio: null,
    estado: "completa",
    criadoEm: "",
    atualizadoEm: "",
    ...parcial,
  };
}

type Gerador = (
  comando: string,
  argumentos: Record<string, unknown>,
  fake: Fake,
) => Promise<ResultadoDaGeracao>;

interface Fake {
  conversas: Conversa[];
  mensagens: Record<string, Mensagem[]>;
}

interface Opcoes {
  conversas?: Conversa[];
  mensagens?: Record<string, Mensagem[]>;
  gerador?: Gerador;
  janelaDeContexto?: number | null;
  mensagensNaExclusao?: number;
}

let proximoId = 1;

/** Um backend de conversas em memória. */
function simularBackend(opcoes: Opcoes = {}): Fake {
  const fake: Fake = {
    conversas: [...(opcoes.conversas ?? [])],
    mensagens: { ...(opcoes.mensagens ?? {}) },
  };
  const padrao: Gerador = async (comando, argumentos, estado) => {
    const id = argumentos.conversaId as string;
    const lista = (estado.mensagens[id] ??= []);
    if (comando === "enviar_mensagem") {
      lista.push(
        mensagem({
          id: `u${proximoId++}`,
          conversaId: id,
          posicao: lista.length + 1,
          papel: "user",
          conteudo: (argumentos.texto as string).trim(),
        }),
      );
    }
    const canal = argumentos.canal as {
      onmessage: (e: EventoDeGeracao) => void;
    };
    canal.onmessage({ tipo: "conteudo", texto: "Olá" });
    canal.onmessage({ tipo: "conteudo", texto: ", mundo!" });
    const resposta = mensagem({
      id: `a${proximoId++}`,
      conversaId: id,
      posicao: lista.length + 1,
      papel: "assistant",
      conteudo: "Olá, mundo!",
    });
    if (comando === "regenerar_resposta") {
      lista[lista.length - 1] = { ...resposta, id: lista[lista.length - 1].id };
    } else {
      lista.push(resposta);
    }
    return { estado: "concluida", mensagem: resposta, cortada: false };
  };

  invoke.mockImplementation(
    async (comando: string, args: Record<string, unknown> = {}) => {
      switch (comando) {
        case "listar_projetos":
          return [projetoA];
        case "listar_modelos_instalados":
          return ["qwen3:8b", "llama3.2:3b"];
        case "detalhes_do_modelo":
          return {
            janelaDeContexto: opcoes.janelaDeContexto ?? null,
            suportaRaciocinio: true,
          };
        case "listar_conversas":
          return fake.conversas.filter((c) => c.projetoId === args.projetoId);
        case "criar_conversa": {
          const nova = conversa({ id: `c${proximoId++}` });
          fake.conversas.unshift(nova);
          return nova;
        }
        case "listar_mensagens":
          return [...(fake.mensagens[args.conversaId as string] ?? [])];
        case "atualizar_conversa": {
          const { id, dados } = args as {
            id: string;
            dados: Partial<Conversa>;
          };
          fake.conversas = fake.conversas.map((c) =>
            c.id === id ? { ...c, ...dados } : c,
          );
          return fake.conversas.find((c) => c.id === id);
        }
        case "resumo_exclusao_conversa":
          return (
            opcoes.mensagensNaExclusao ??
            (fake.mensagens[args.id as string] ?? []).length
          );
        case "excluir_conversa":
          fake.conversas = fake.conversas.filter((c) => c.id !== args.id);
          return undefined;
        case "cancelar_geracao":
          return undefined;
        case "enviar_mensagem":
        case "gerar_resposta_pendente":
        case "regenerar_resposta":
          return (opcoes.gerador ?? padrao)(comando, args, fake);
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

async function abrirConversa(titulo: string) {
  const lateral = await screen.findByRole("list", {
    name: "Conversas de Projeto A",
  });
  await clicar(within(lateral).getByRole("button", { name: titulo }));
  return screen.findByRole("region", { name: "Conversa" });
}

async function escrever(texto: string) {
  fireEvent.change(await screen.findByLabelText("Mensagem"), {
    target: { value: texto },
  });
}

async function enviar(texto: string) {
  await escrever(texto);
  await clicar(botao("Enviar"));
}

function renderizar() {
  render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
}

function promessaControlavel<T>() {
  let resolver!: (valor: T) => void;
  let rejeitar!: (motivo: unknown) => void;
  const promessa = new Promise<T>((resolve, reject) => {
    resolver = resolve;
    rejeitar = reject;
  });
  return { promessa, resolver, rejeitar };
}

beforeEach(() => {
  invoke.mockReset();
  canais.length = 0;
  reiniciarProjetos();
  reiniciarConversas();
  proximoId = 1;
});

describe("Conversas: lista e criação", () => {
  it("mostra as conversas do projeto na lateral, da mais recente para a mais antiga", async () => {
    simularBackend({
      conversas: [
        conversa({ id: "c2", titulo: "Recente" }),
        conversa({ id: "c1", titulo: "Antiga" }),
      ],
    });
    renderizar();

    const lista = await screen.findByRole("list", {
      name: "Conversas de Projeto A",
    });
    await within(lista).findByRole("button", { name: "Recente" });
    const titulos = within(lista)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(titulos).toEqual(["Recente", "Antiga", "+ Nova conversa"]);
  });

  it("cria uma conversa dentro do projeto e a abre", async () => {
    simularBackend();
    renderizar();
    const lista = await screen.findByRole("list", {
      name: "Conversas de Projeto A",
    });

    await clicar(within(lista).getByRole("button", { name: /Nova conversa/ }));

    expect(chamadas("criar_conversa")[0][1]).toEqual({ projetoId: "p-a" });
    expect(
      await screen.findByRole("region", { name: "Conversa" }),
    ).toBeVisible();
    expect(
      screen.getByText("Escreva a primeira mensagem para começar."),
    ).toBeVisible();
  });

  it("só uma conversa fica ativa por vez", async () => {
    simularBackend({
      conversas: [
        conversa({ id: "c1", titulo: "Primeira" }),
        conversa({ id: "c2", titulo: "Segunda" }),
      ],
    });
    renderizar();
    const lista = await screen.findByRole("list", {
      name: "Conversas de Projeto A",
    });

    await abrirConversa("Primeira");
    expect(
      within(lista).getByRole("button", { name: "Primeira" }),
    ).toHaveAttribute("aria-current", "true");

    await abrirConversa("Segunda");

    expect(
      within(lista).getByRole("button", { name: "Segunda" }),
    ).toHaveAttribute("aria-current", "true");
    expect(
      within(lista).getByRole("button", { name: "Primeira" }),
    ).not.toHaveAttribute("aria-current");
    expect(screen.getAllByRole("region", { name: "Conversa" })).toHaveLength(1);
  });

  it("abrir uma conversa mostra o histórico que estava guardado", async () => {
    simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Com histórico" })],
      mensagens: {
        c1: [
          mensagem({ id: "m1", conteudo: "Pergunta antiga" }),
          mensagem({
            id: "m2",
            posicao: 2,
            papel: "assistant",
            conteudo: "Resposta guardada",
          }),
        ],
      },
    });
    renderizar();

    await abrirConversa("Com histórico");

    expect(await screen.findByText("Pergunta antiga")).toBeVisible();
    expect(screen.getByText("Resposta guardada")).toBeVisible();
  });
});

describe("Conversas: mensagens, markdown e raciocínio", () => {
  async function abrirComResposta(resposta: Partial<Mensagem>) {
    simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Teste" })],
      mensagens: {
        c1: [
          mensagem({ id: "m1", conteudo: "Pergunta" }),
          mensagem({
            id: "m2",
            posicao: 2,
            papel: "assistant",
            conteudo: "Resposta",
            ...resposta,
          }),
        ],
      },
    });
    renderizar();
    await abrirConversa("Teste");
    await screen.findByText("Pergunta");
  }

  it("renderiza markdown: negrito, listas, tabelas e código com realce", async () => {
    await abrirComResposta({
      conteudo: [
        "Isto é **negrito** e `código`.",
        "",
        "- item um",
        "- item dois",
        "",
        "| Nome | Valor |",
        "| --- | --- |",
        "| a | 1 |",
        "",
        "```rust",
        'fn main() { println!("oi"); }',
        "```",
      ].join("\n"),
    });

    const resposta = await screen.findByRole("article", { name: "Assistente" });
    expect(within(resposta).getByText("negrito").tagName).toBe("STRONG");
    expect(within(resposta).getAllByRole("listitem")).toHaveLength(2);
    expect(within(resposta).getByRole("table")).toBeVisible();
    const codigo = resposta.querySelector("pre code");
    expect(codigo?.className).toContain("language-rust");
    expect(codigo?.querySelector(".hljs-keyword")).not.toBeNull();
  });

  it("links e imagens aparecem como texto, sem nada clicável nem carregado", async () => {
    await abrirComResposta({
      conteudo:
        "Veja [o site](https://exemplo.com/pagina) e https://auto.exemplo.com.\n\n![gato](https://exemplo.com/gato.png)",
    });

    const resposta = await screen.findByRole("article", { name: "Assistente" });
    expect(resposta.querySelector("a")).toBeNull();
    expect(resposta.querySelector("img")).toBeNull();
    expect(resposta.textContent).toContain(
      "o site (https://exemplo.com/pagina)",
    );
    expect(resposta.textContent).toContain("https://auto.exemplo.com");
    expect(resposta.textContent).toContain("[imagem: gato]");
  });

  it("HTML cru na resposta não vira elemento da página", async () => {
    await abrirComResposta({
      conteudo:
        'Olá <script>window.invadido = true</script><img src="x" onerror="alert(1)"><b>negrito html</b>',
    });

    const resposta = await screen.findByRole("article", { name: "Assistente" });
    expect(resposta.querySelector("script")).toBeNull();
    expect(resposta.querySelector("img")).toBeNull();
    expect(resposta.querySelector("b")).toBeNull();
    expect(
      (window as unknown as { invadido?: boolean }).invadido,
    ).toBeUndefined();
  });

  it("uma resposta com raciocínio mostra o bloco recolhido", async () => {
    await abrirComResposta({ raciocinio: "Primeiro pensei nisto." });

    const resumo = await screen.findByText("Raciocínio");
    const bloco = resumo.closest("details");
    expect(bloco).not.toBeNull();
    expect(bloco).not.toHaveAttribute("open");
    expect(bloco).toHaveTextContent("Primeiro pensei nisto.");
  });

  it("uma resposta sem raciocínio não mostra a seção", async () => {
    await abrirComResposta({ raciocinio: null });

    await screen.findByRole("article", { name: "Assistente" });
    expect(screen.queryByText("Raciocínio")).not.toBeInTheDocument();
    expect(document.querySelector("details summary")?.textContent).not.toBe(
      "Raciocínio",
    );
  });

  it("marca a resposta interrompida", async () => {
    await abrirComResposta({ estado: "interrompida", conteudo: "Pela metade" });

    expect(await screen.findByText("Resposta interrompida.")).toBeVisible();
  });
});

describe("Conversas: envio com streaming", () => {
  async function abrirVazia(opcoes: Opcoes = {}) {
    const fake = simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Chat" })],
      ...opcoes,
    });
    renderizar();
    await abrirConversa("Chat");
    return fake;
  }

  it("mostra o texto chegando em streaming e grava a resposta ao terminar", async () => {
    const geracao = promessaControlavel<ResultadoDaGeracao>();
    const fake = await abrirVazia({
      gerador: async (_comando, args, estado) => {
        const lista = (estado.mensagens.c1 ??= []);
        lista.push(
          mensagem({ id: "u1", papel: "user", conteudo: args.texto as string }),
        );
        return geracao.promessa;
      },
    });

    await enviar("Olá, modelo");

    // A mensagem do usuário aparece já, e a resposta vai chegando aos pedaços.
    expect(
      within(screen.getByRole("article", { name: "Você" })).getByText(
        "Olá, modelo",
      ),
    ).toBeVisible();
    expect(screen.getByText("Gerando a resposta…")).toBeVisible();
    expect(botao("Parar")).toBeVisible();
    const canal = canais[0];
    act(() =>
      canal.onmessage({ tipo: "raciocinio", texto: "Pensando um pouco." }),
    );
    act(() => canal.onmessage({ tipo: "conteudo", texto: "Primeira parte" }));
    expect(screen.getByText("Primeira parte")).toBeVisible();
    expect(screen.getByText("Raciocínio (pensando…)")).toBeVisible();
    act(() => canal.onmessage({ tipo: "conteudo", texto: " e a segunda." }));
    expect(screen.getByText("Primeira parte e a segunda.")).toBeVisible();

    fake.mensagens.c1.push(
      mensagem({
        id: "a1",
        posicao: 2,
        papel: "assistant",
        conteudo: "Primeira parte e a segunda.",
        raciocinio: "Pensando um pouco.",
      }),
    );
    await act(async () =>
      geracao.resolver({ estado: "concluida", mensagem: null, cortada: false }),
    );

    expect(screen.queryByText("Gerando a resposta…")).not.toBeInTheDocument();
    expect(screen.getByText("Primeira parte e a segunda.")).toBeVisible();
    expect(screen.getByLabelText("Mensagem")).toHaveValue("");
    expect(chamadas("enviar_mensagem")[0][1]).toMatchObject({
      conversaId: "c1",
      texto: "Olá, modelo",
    });
  });

  it("Enter envia e Shift+Enter só quebra a linha", async () => {
    await abrirVazia();
    const campo = await screen.findByLabelText("Mensagem");
    fireEvent.change(campo, { target: { value: "Linha 1" } });

    fireEvent.keyDown(campo, { key: "Enter", shiftKey: true });
    expect(chamadas("enviar_mensagem")).toHaveLength(0);

    await act(async () => {
      fireEvent.keyDown(campo, { key: "Enter" });
    });
    expect(chamadas("enviar_mensagem")).toHaveLength(1);
  });

  it("mensagem em branco não é enviada", async () => {
    await abrirVazia();
    await escrever("   ");

    expect(botao("Enviar")).toBeDisabled();
    fireEvent.keyDown(screen.getByLabelText("Mensagem"), { key: "Enter" });
    expect(chamadas("enviar_mensagem")).toHaveLength(0);
  });

  it("sem modelo escolhido não envia e explica", async () => {
    simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Chat", modelo: null })],
    });
    renderizar();
    await abrirConversa("Chat");
    await escrever("Oi");

    expect(botao("Enviar")).toBeDisabled();
    expect(screen.getByText(/Escolha um modelo em/)).toBeVisible();
  });

  it("parar a geração pede o cancelamento", async () => {
    await abrirVazia({ gerador: () => new Promise(() => undefined) });
    await enviar("Oi");

    await clicar(botao("Parar"));

    expect(chamadas("cancelar_geracao")).toHaveLength(1);
  });

  it("durante a geração não dá para trocar de conversa nem de projeto", async () => {
    await abrirVazia({ gerador: () => new Promise(() => undefined) });
    await enviar("Oi");

    expect(screen.getByRole("button", { name: "Chat" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Projeto A" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Novo projeto" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: /Nova conversa/ }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Excluir conversa" }),
    ).toBeDisabled();
  });

  it("com o Ollama fora do ar mostra a mensagem amigável, mantém a mensagem e deixa tentar de novo", async () => {
    let tentativa = 0;
    const fake = await abrirVazia({
      gerador: async (comando, args, estado) => {
        tentativa += 1;
        const lista = (estado.mensagens.c1 ??= []);
        if (comando === "enviar_mensagem") {
          lista.push(
            mensagem({
              id: "u1",
              papel: "user",
              conteudo: args.texto as string,
            }),
          );
        }
        if (tentativa === 1) {
          throw {
            mensagem:
              "O Ollama não está respondendo. Verifique se ele está em execução.",
            mensagemParcial: null,
          };
        }
        lista.push(
          mensagem({
            id: "a1",
            posicao: 2,
            papel: "assistant",
            conteudo: "Voltei.",
          }),
        );
        return { estado: "concluida", mensagem: null, cortada: false };
      },
    });

    await enviar("Não perca isto");

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("O Ollama não está respondendo.");
    expect(screen.getByText("Não perca isto")).toBeVisible();
    expect(fake.mensagens.c1).toHaveLength(1);

    await clicar(
      within(alerta).getByRole("button", { name: "Tentar de novo" }),
    );

    expect(chamadas("gerar_resposta_pendente")).toHaveLength(1);
    expect(await screen.findByText("Voltei.")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getAllByText("Não perca isto")).toHaveLength(1);
  });

  it("uma mensagem recusada antes de ser gravada continua no campo para ser corrigida", async () => {
    await abrirVazia({
      gerador: async () => {
        throw {
          mensagem: "A mensagem é grande demais.",
          mensagemParcial: null,
        };
      },
    });

    await enviar("Texto que será recusado");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "A mensagem é grande demais.",
    );
    expect(screen.getByLabelText("Mensagem")).toHaveValue(
      "Texto que será recusado",
    );
    expect(
      screen.queryByText("Texto que será recusado", { selector: "p" }),
    ).toBeNull();
  });

  it("não mostra o erro cru quando o comando falha de outro jeito", async () => {
    await abrirVazia({
      gerador: async (_comando, args, estado) => {
        (estado.mensagens.c1 ??= []).push(
          mensagem({ id: "u1", papel: "user", conteudo: args.texto as string }),
        );
        throw new TypeError("Cannot read properties of undefined");
      },
    });

    await enviar("Oi");

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível concluir.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });

  it("avisa quando a resposta foi cortada por limite de tamanho", async () => {
    await abrirVazia({
      gerador: async (_comando, _args, estado) => {
        const lista = (estado.mensagens.c1 ??= []);
        lista.push(mensagem({ id: "u1", papel: "user", conteudo: "Oi" }));
        lista.push(
          mensagem({
            id: "a1",
            posicao: 2,
            papel: "assistant",
            conteudo: "Texto lon",
          }),
        );
        return { estado: "concluida", mensagem: null, cortada: true };
      },
    });

    await enviar("Oi");

    expect(
      await screen.findByText(/chegou ao limite de tamanho/),
    ).toBeVisible();
  });

  it("depois de uma falha no meio, a resposta interrompida oferece tentar de novo (gerar outra)", async () => {
    let tentativa = 0;
    await abrirVazia({
      gerador: async (_comando, _args, estado) => {
        tentativa += 1;
        const lista = (estado.mensagens.c1 ??= []);
        if (tentativa === 1) {
          lista.push(
            mensagem({ id: "u1", papel: "user", conteudo: "Pergunta" }),
            mensagem({
              id: "a1",
              posicao: 2,
              papel: "assistant",
              conteudo: "Resposta pela",
              estado: "interrompida",
            }),
          );
          throw {
            mensagem: "A conexão com o Ollama caiu no meio da operação.",
            mensagemParcial: lista[1],
          };
        }
        lista[1] = {
          ...lista[1],
          conteudo: "Resposta inteira.",
          estado: "completa",
        };
        return { estado: "concluida", mensagem: null, cortada: false };
      },
    });

    await enviar("Pergunta");

    const alerta = await screen.findByRole("alert");
    expect(screen.getByText("Resposta pela")).toBeVisible();
    expect(screen.getByText("Resposta interrompida.")).toBeVisible();
    await clicar(
      within(alerta).getByRole("button", { name: "Tentar de novo" }),
    );

    expect(chamadas("regenerar_resposta")).toHaveLength(1);
    expect(await screen.findByText("Resposta inteira.")).toBeVisible();
    expect(
      screen.queryByText("Resposta interrompida."),
    ).not.toBeInTheDocument();
  });
});

describe("Conversas: regenerar", () => {
  it("só a última resposta tem o botão, e regenerar a substitui", async () => {
    const fake = simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Chat" })],
      mensagens: {
        c1: [
          mensagem({ id: "u1", conteudo: "Primeira" }),
          mensagem({
            id: "a1",
            posicao: 2,
            papel: "assistant",
            conteudo: "Resposta antiga A",
          }),
          mensagem({ id: "u2", posicao: 3, conteudo: "Segunda" }),
          mensagem({
            id: "a2",
            posicao: 4,
            papel: "assistant",
            conteudo: "Resposta antiga B",
          }),
        ],
      },
      gerador: async (_comando, _args, estado) => {
        const lista = estado.mensagens.c1;
        lista[lista.length - 1] = {
          ...lista[lista.length - 1],
          conteudo: "Resposta nova B",
        };
        return { estado: "concluida", mensagem: null, cortada: false };
      },
    });
    renderizar();
    await abrirConversa("Chat");
    await screen.findByText("Resposta antiga B");

    expect(
      screen.getAllByRole("button", { name: "Gerar de novo" }),
    ).toHaveLength(1);
    await clicar(botao("Gerar de novo"));

    expect(chamadas("regenerar_resposta")).toHaveLength(1);
    expect(await screen.findByText("Resposta nova B")).toBeVisible();
    expect(screen.queryByText("Resposta antiga B")).not.toBeInTheDocument();
    expect(screen.getByText("Resposta antiga A")).toBeVisible();
    expect(fake.mensagens.c1).toHaveLength(4);
  });
});

describe("Conversas: modelo, parâmetros e título", () => {
  async function abrirParametros(opcoes: Opcoes = {}) {
    const fake = simularBackend({
      conversas: [
        conversa({
          id: "c1",
          titulo: "Chat",
          temperature: 0.7,
          numCtx: 8192,
          keepAlive: "5m",
        }),
        conversa({
          id: "c2",
          titulo: "Outra",
          modelo: "llama3.2:3b",
          temperature: 1.5,
          numCtx: 4096,
          keepAlive: "-1",
        }),
      ],
      ...opcoes,
    });
    renderizar();
    await abrirConversa("Chat");
    await clicar(screen.getByText("Modelo e parâmetros"));
    return fake;
  }

  it("mostra o modelo e os parâmetros da conversa e salva cada mudança", async () => {
    await abrirParametros();
    await screen.findByRole("option", { name: "llama3.2:3b" });

    expect(screen.getByLabelText("Modelo")).toHaveValue("qwen3:8b");
    expect(screen.getByLabelText(/Temperatura/)).toHaveValue(0.7);
    expect(screen.getByLabelText(/Contexto/)).toHaveValue("8192");
    expect(screen.getByLabelText(/Manter o modelo/)).toHaveValue("5m");

    fireEvent.change(screen.getByLabelText("Modelo"), {
      target: { value: "llama3.2:3b" },
    });
    await screen.findByDisplayValue("llama3.2:3b");
    fireEvent.change(screen.getByLabelText(/Temperatura/), {
      target: { value: "1.2" },
    });
    fireEvent.change(screen.getByLabelText(/Contexto/), {
      target: { value: "16384" },
    });
    fireEvent.change(screen.getByLabelText(/Manter o modelo/), {
      target: { value: "1h" },
    });

    const ultimas = chamadas("atualizar_conversa").map(([, args]) => args);
    expect(ultimas[0]).toMatchObject({
      id: "c1",
      dados: { modelo: "llama3.2:3b" },
    });
    expect(ultimas[1]).toMatchObject({ dados: { temperature: 1.2 } });
    expect(ultimas[2]).toMatchObject({ dados: { numCtx: 16384 } });
    expect(ultimas[3]).toMatchObject({ dados: { keepAlive: "1h" } });
  });

  it("cada conversa mostra os próprios valores, e mudar uma não altera a outra", async () => {
    const fake = await abrirParametros();
    await screen.findByRole("option", { name: "llama3.2:3b" });
    fireEvent.change(screen.getByLabelText(/Temperatura/), {
      target: { value: "0.2" },
    });
    await screen.findByDisplayValue("0.2");

    await abrirConversa("Outra");
    await screen.findByLabelText("Modelo");

    expect(screen.getByLabelText("Modelo")).toHaveValue("llama3.2:3b");
    expect(screen.getByLabelText(/Temperatura/)).toHaveValue(1.5);
    expect(screen.getByLabelText(/Contexto/)).toHaveValue("4096");
    expect(screen.getByLabelText(/Manter o modelo/)).toHaveValue("-1");
    expect(fake.conversas.find((c) => c.id === "c2")?.temperature).toBe(1.5);
    expect(fake.conversas.find((c) => c.id === "c1")?.temperature).toBe(0.2);
  });

  it("o contexto oferecido é limitado à janela máxima do modelo", async () => {
    await abrirParametros({ janelaDeContexto: 16384 });
    await screen.findByRole("option", { name: "llama3.2:3b" });

    const opcoes = within(screen.getByLabelText(/Contexto/))
      .getAllByRole("option")
      .map((o) => o.getAttribute("value"));
    expect(opcoes).toEqual(["", "2048", "4096", "8192", "16384"]);
  });

  it("uma mudança recusada mostra a mensagem amigável", async () => {
    await abrirParametros();
    await screen.findByRole("option", { name: "llama3.2:3b" });
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation(
      (comando: string, args?: Record<string, unknown>) =>
        comando === "atualizar_conversa"
          ? Promise.reject("A temperatura precisa estar entre 0 e 2.")
          : original(comando, args),
    );

    fireEvent.change(screen.getByLabelText(/Temperatura/), {
      target: { value: "5" },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "A temperatura precisa estar entre 0 e 2.",
    );
  });

  it("renomeia a conversa e recusa título vazio", async () => {
    simularBackend({ conversas: [conversa({ id: "c1", titulo: "Chat" })] });
    renderizar();
    await abrirConversa("Chat");

    await clicar(botao("Renomear"));
    fireEvent.change(screen.getByLabelText("Título da conversa"), {
      target: { value: "  " },
    });
    await clicar(botao("Salvar título"));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Dê um título à conversa.",
    );
    expect(chamadas("atualizar_conversa")).toHaveLength(0);

    fireEvent.change(screen.getByLabelText("Título da conversa"), {
      target: { value: "Meu título" },
    });
    await clicar(botao("Salvar título"));

    expect(chamadas("atualizar_conversa")[0][1]).toMatchObject({
      id: "c1",
      dados: { titulo: "Meu título" },
    });
    expect(
      await screen.findByRole("heading", { name: "Meu título" }),
    ).toBeVisible();
  });
});

describe("Conversas: excluir (com confirmação)", () => {
  async function abrirParaExcluir() {
    simularBackend({
      conversas: [conversa({ id: "c1", titulo: "Para excluir" })],
      mensagens: {
        c1: [
          mensagem({ id: "m1", conteudo: "Oi" }),
          mensagem({
            id: "m2",
            posicao: 2,
            papel: "assistant",
            conteudo: "Olá",
          }),
        ],
      },
    });
    renderizar();
    await abrirConversa("Para excluir");
    await clicar(
      await screen.findByRole("button", { name: "Excluir conversa" }),
    );
    return screen.findByRole("alertdialog");
  }

  it("pede confirmação e diz quantas mensagens serão apagadas, sem apagar ainda", async () => {
    const dialogo = await abrirParaExcluir();

    expect(
      within(dialogo).getByText(/Excluir a conversa "Para excluir"\?/),
    ).toBeVisible();
    expect(
      await within(dialogo).findByText(/Isto apaga 2 mensagens/),
    ).toBeVisible();
    expect(chamadas("excluir_conversa")).toHaveLength(0);
  });

  it("cancelar não apaga nada", async () => {
    const dialogo = await abrirParaExcluir();

    await clicar(within(dialogo).getByRole("button", { name: "Cancelar" }));

    expect(chamadas("excluir_conversa")).toHaveLength(0);
    expect(screen.getByRole("region", { name: "Conversa" })).toBeVisible();
  });

  it("confirmar apaga a conversa e volta ao projeto", async () => {
    const dialogo = await abrirParaExcluir();
    await within(dialogo).findByText(/Isto apaga 2 mensagens/);

    await clicar(
      within(dialogo).getByRole("button", { name: "Excluir conversa" }),
    );

    expect(chamadas("excluir_conversa")[0][1]).toEqual({ id: "c1" });
    expect(
      await screen.findByRole("heading", { name: "Projeto A" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Para excluir" }),
    ).not.toBeInTheDocument();
  });
});
