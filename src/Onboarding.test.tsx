import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Onboarding from "./Onboarding";
import type { EstadoOllama } from "./tipos";

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

const GB = 1024 ** 3;

const catalogo = {
  versao: 1,
  atualizadoEm: "2026-11-08",
  modelos: [
    {
      tag: "llama3.2:3b",
      nome: "Llama 3.2 (3B)",
      descricao: "Pequeno e rápido.",
      faixa: "leve",
      tamanhoEmBytes: 2 * GB,
      ramEstimadaEmGb: 4,
      raciocinio: false,
    },
    {
      tag: "qwen3:8b",
      nome: "Qwen 3 (8B)",
      descricao: "Respostas completas.",
      faixa: "equilibrado",
      tamanhoEmBytes: 5 * GB,
      ramEstimadaEmGb: 10,
      raciocinio: true,
    },
  ],
};

const suficiente = {
  estado: "suficiente",
  livreEmBytes: 100 * GB,
  necessarioEmBytes: 2 * GB,
  mensagem: null,
};

function ollama(parcial: Partial<EstadoOllama> = {}): EstadoOllama {
  return {
    estado: "em_execucao",
    versao: "0.32.9",
    origem: "externo",
    modelosInstalados: 0,
    aviso: null,
    mensagem: null,
    ...parcial,
  };
}

type Resposta = unknown | ((args: unknown) => unknown);

/** Simula os comandos do backend. Um comando sem resposta definida falha o teste. */
function simular(respostas: Record<string, Resposta>) {
  invoke.mockImplementation((comando: string, args: unknown) => {
    if (!(comando in respostas)) {
      return Promise.reject(`comando inesperado: ${comando}`);
    }
    const resposta = respostas[comando];
    const valor = typeof resposta === "function" ? resposta(args) : resposta;
    return valor instanceof Promise ? valor : Promise.resolve(valor);
  });
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

async function clicar(elemento: HTMLElement) {
  await act(async () => {
    fireEvent.click(elemento);
  });
}

function botao(nome: string | RegExp) {
  return screen.getByRole("button", { name: nome });
}

function renderizar(
  props: Partial<React.ComponentProps<typeof Onboarding>> = {},
) {
  const aoAtualizarOllama = vi.fn();
  const aoConcluir = vi.fn();
  render(
    <Onboarding
      ollama={ollama()}
      jaConcluiuAntes={false}
      aoAtualizarOllama={aoAtualizarOllama}
      aoConcluir={aoConcluir}
      {...props}
    />,
  );
  return { aoAtualizarOllama, aoConcluir };
}

/** Do início até a lista de modelos, com o Ollama rodando e sem modelos. */
async function irAteEscolherModelo() {
  await clicar(botao("Continuar")); // boas-vindas
  await clicar(botao("Continuar")); // ollama
  await screen.findByText("Escolha um modelo");
}

describe("Onboarding: passos iniciais", () => {
  beforeEach(() => {
    invoke.mockReset();
    canais.length = 0;
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
    });
  });

  it("começa pelas boas-vindas, com o aviso de independência em relação ao Ollama", async () => {
    renderizar();

    expect(
      screen.getByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).toBeVisible();
    expect(
      screen.getByText(/O Memoteca é independente e não é afiliado a ele/),
    ).toBeVisible();

    await clicar(botao("Continuar"));

    expect(screen.getByRole("heading", { name: "O Ollama" })).toBeVisible();
  });

  it("quem já concluiu antes pula as boas-vindas", () => {
    renderizar({ jaConcluiuAntes: true });

    expect(screen.getByRole("heading", { name: "O Ollama" })).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Bem-vindo ao Memoteca" }),
    ).not.toBeInTheDocument();
  });

  it("com o Ollama pronto e sem modelos, leva à escolha de modelo", async () => {
    renderizar();

    await clicar(botao("Continuar")); // boas-vindas
    expect(screen.getByText("Ollama 0.32.9 em execução.")).toBeVisible();
    await clicar(botao("Continuar")); // ollama

    expect(await screen.findByText("Escolha um modelo")).toBeVisible();
  });

  it("com o Ollama pronto e com modelos, vai direto para o pronto", async () => {
    renderizar({ ollama: ollama({ modelosInstalados: 3 }) });

    await clicar(botao("Continuar"));
    await clicar(botao("Continuar"));

    expect(screen.getByRole("heading", { name: "Tudo pronto" })).toBeVisible();
  });

  it("mostra o aviso de versão antiga sem bloquear", async () => {
    renderizar({
      ollama: ollama({ aviso: "Atualize o Ollama para a versão 0.30.0." }),
      jaConcluiuAntes: true,
    });

    expect(
      screen.getByText("Atualize o Ollama para a versão 0.30.0."),
    ).toBeVisible();
    expect(botao("Continuar")).toBeEnabled();
  });

  it("sem Ollama instalado, oferece instalar com uma explicação", () => {
    renderizar({
      ollama: ollama({ estado: "nao_instalado", versao: null }),
      jaConcluiuAntes: true,
    });

    expect(screen.getByText(/ainda não está instalado/)).toBeVisible();
    expect(botao("Instalar o Ollama")).toBeVisible();
  });

  it("com outro programa na porta, mostra a mensagem amigável e deixa verificar de novo", async () => {
    const { aoAtualizarOllama } = renderizar({
      ollama: ollama({
        estado: "porta_ocupada",
        versao: null,
        mensagem: "Outro programa está usando a porta do Ollama (11434).",
      }),
      jaConcluiuAntes: true,
    });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Outro programa está usando a porta do Ollama (11434).",
    );
    await clicar(botao("Verificar de novo"));

    expect(aoAtualizarOllama).toHaveBeenCalledTimes(1);
  });

  it("enquanto o Ollama é verificado, mostra que está verificando", () => {
    renderizar({ ollama: null, jaConcluiuAntes: true });

    expect(screen.getByText("Verificando o Ollama…")).toBeVisible();
  });
});

describe("Onboarding: escolha do modelo", () => {
  beforeEach(() => {
    invoke.mockReset();
    canais.length = 0;
  });

  it("mostra o catálogo por faixa, com tamanho, memória estimada e raciocínio", async () => {
    simular({ catalogo_de_modelos: catalogo });
    renderizar();

    await irAteEscolherModelo();

    expect(screen.getByText("Leves (computadores simples)")).toBeVisible();
    expect(screen.getByText("Equilibrados")).toBeVisible();
    expect(screen.queryByText(/Potentes/)).not.toBeInTheDocument();
    expect(screen.getByText("Llama 3.2 (3B)")).toBeVisible();
    expect(
      screen.getByText(/2,0 GB de download, cerca de 4 GB de memória/),
    ).toBeVisible();
    expect(screen.getByText(/com raciocínio/)).toBeVisible();
    expect(screen.getByRole("textbox", { name: /Outro modelo/ })).toBeVisible();
    expect(botao("Baixar")).toBeDisabled();
  });

  it("mostra uma mensagem amigável se o catálogo não carregar", async () => {
    simular({
      catalogo_de_modelos: () =>
        Promise.reject("Não foi possível carregar a lista de modelos."),
    });
    renderizar();

    await clicar(botao("Continuar"));
    await clicar(botao("Continuar"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível carregar a lista de modelos.",
    );
  });

  it("espaço insuficiente bloqueia o botão e explica", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: {
        estado: "insuficiente",
        livreEmBytes: GB,
        necessarioEmBytes: 2 * GB,
        mensagem:
          "Não há espaço livre suficiente para baixar este modelo. São necessários 2,0 GB e há 1,0 GB livres.",
      },
    });
    renderizar();
    await irAteEscolherModelo();

    await clicar(screen.getByRole("radio", { name: /Llama 3.2/ }));

    expect(invoke).toHaveBeenCalledWith("avaliar_espaco_do_modelo", {
      tag: "llama3.2:3b",
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não há espaço livre suficiente",
    );
    expect(botao("Baixar")).toBeDisabled();
  });

  it("pouca folga só deixa baixar depois de o usuário confirmar", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: {
        estado: "alerta",
        livreEmBytes: 3 * GB,
        necessarioEmBytes: 2 * GB,
        mensagem: "Depois do download, sobrariam só 1,0 GB livres no disco.",
      },
    });
    renderizar();
    await irAteEscolherModelo();

    await clicar(screen.getByRole("radio", { name: /Llama 3.2/ }));

    expect(await screen.findByText(/sobrariam só 1,0 GB livres/)).toBeVisible();
    expect(botao("Baixar")).toBeDisabled();

    await clicar(screen.getByRole("checkbox", { name: /baixar mesmo assim/ }));

    expect(botao("Baixar")).toBeEnabled();
  });

  it("um modelo digitado à mão avisa que o tamanho só é conhecido ao começar", async () => {
    simular({ catalogo_de_modelos: catalogo });
    renderizar();
    await irAteEscolherModelo();

    fireEvent.change(screen.getByRole("textbox", { name: /Outro modelo/ }), {
      target: { value: "llama3.2:1b" },
    });

    expect(screen.getByRole("note")).toHaveTextContent(
      "O tamanho só é conhecido quando o download começa",
    );
    expect(botao("Baixar")).toBeEnabled();
  });

  it("escolher um do catálogo limpa o campo livre, e digitar desmarca o catálogo", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
    });
    renderizar();
    await irAteEscolherModelo();
    const campo = screen.getByRole("textbox", { name: /Outro modelo/ });

    fireEvent.change(campo, { target: { value: "abc" } });
    await clicar(screen.getByRole("radio", { name: /Qwen 3/ }));
    expect(campo).toHaveValue("");

    fireEvent.change(campo, { target: { value: "xyz" } });
    expect(screen.getByRole("radio", { name: /Qwen 3/ })).not.toBeChecked();
  });
});

describe("Onboarding: download e conclusão", () => {
  beforeEach(() => {
    invoke.mockReset();
    canais.length = 0;
  });

  async function escolherEBaixar(tag = /Llama 3.2/) {
    await irAteEscolherModelo();
    await clicar(screen.getByRole("radio", { name: tag }));
    await clicar(botao("Baixar"));
  }

  it("leva o usuário do início até o app, só pela interface", async () => {
    const download = promessaControlavel<string>();
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
      baixar_modelo: () => download.promessa,
      concluir_onboarding: undefined,
    });
    const { aoAtualizarOllama, aoConcluir } = renderizar();

    await escolherEBaixar();

    expect(invoke).toHaveBeenCalledWith("baixar_modelo", {
      nome: "llama3.2:3b",
      canal: canais[0],
    });
    expect(
      screen.getByRole("heading", { name: /Baixando llama3.2:3b/ }),
    ).toBeVisible();
    expect(screen.getByText("Preparando o download…")).toBeVisible();

    act(() =>
      canais[0].onmessage({
        fase: "baixando",
        baixadoEmBytes: GB,
        totalEmBytes: 2 * GB,
      }),
    );
    expect(screen.getByText("Baixando o modelo…")).toBeVisible();
    expect(screen.getByRole("progressbar")).toHaveValue(50);
    expect(screen.getByText(/50% \(1,0 GB de 2,0 GB\)/)).toBeVisible();

    await act(async () => download.resolver("concluido"));

    expect(aoAtualizarOllama).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("heading", { name: "Tudo pronto" })).toBeVisible();

    await clicar(botao("Começar"));

    expect(invoke).toHaveBeenCalledWith("concluir_onboarding");
    expect(aoConcluir).toHaveBeenCalledTimes(1);
  });

  it("começa o download uma única vez", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
      baixar_modelo: () => new Promise(() => undefined),
    });
    renderizar();

    await escolherEBaixar();

    const chamadas = invoke.mock.calls.filter(([c]) => c === "baixar_modelo");
    expect(chamadas).toHaveLength(1);
  });

  it("baixa um modelo digitado à mão com o nome digitado", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      baixar_modelo: () => new Promise(() => undefined),
    });
    renderizar();
    await irAteEscolherModelo();

    fireEvent.change(screen.getByRole("textbox", { name: /Outro modelo/ }), {
      target: { value: "  meu/modelo:1b  " },
    });
    await clicar(botao("Baixar"));

    expect(invoke).toHaveBeenCalledWith(
      "baixar_modelo",
      expect.objectContaining({ nome: "meu/modelo:1b" }),
    );
  });

  it("download interrompido mostra a mensagem amigável e oferece continuar", async () => {
    let chamada = 0;
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
      baixar_modelo: () => {
        chamada += 1;
        return chamada === 1
          ? Promise.reject({
              mensagem:
                "O download foi interrompido, mas o que já foi baixado foi guardado. Clique em Continuar para retomar de onde parou.",
              podeContinuar: true,
            })
          : Promise.resolve("concluido");
      },
    });
    renderizar();

    await escolherEBaixar();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "o que já foi baixado foi guardado",
    );
    expect(botao("Continuar o download")).toBeVisible();

    await clicar(botao("Continuar o download"));

    expect(chamada).toBe(2);
    expect(screen.getByRole("heading", { name: "Tudo pronto" })).toBeVisible();
  });

  it("modelo inexistente mostra a mensagem amigável e deixa escolher outro", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      baixar_modelo: () =>
        Promise.reject({
          mensagem:
            "Não encontramos esse modelo na biblioteca do Ollama. Confira o nome e tente de novo.",
          podeContinuar: false,
        }),
    });
    renderizar();
    await irAteEscolherModelo();
    fireEvent.change(screen.getByRole("textbox", { name: /Outro modelo/ }), {
      target: { value: "modelo-que-nao-existe:9b" },
    });
    await clicar(botao("Baixar"));

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Confira o nome");
    expect(alerta).not.toHaveTextContent("manifest");
    expect(botao("Tentar de novo")).toBeVisible();

    await clicar(botao("Escolher outro modelo"));

    expect(screen.getByText("Escolha um modelo")).toBeVisible();
  });

  it("sem espaço durante o download, mostra a mensagem e não oferece continuar", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      baixar_modelo: () =>
        Promise.reject({
          mensagem:
            "Não há espaço livre suficiente para baixar este modelo. São necessários 20,0 GB e há 2,0 GB livres.",
          podeContinuar: false,
        }),
    });
    renderizar();
    await irAteEscolherModelo();
    fireEvent.change(screen.getByRole("textbox", { name: /Outro modelo/ }), {
      target: { value: "alguem/modelo-grande:70b" },
    });
    await clicar(botao("Baixar"));

    expect(await screen.findByRole("alert")).toHaveTextContent("espaço livre");
    expect(botao("Tentar de novo")).toBeVisible();
  });

  it("não mostra o erro cru quando a chamada falha de outro jeito", async () => {
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
      baixar_modelo: () =>
        Promise.reject(new TypeError("Cannot read properties")),
    });
    renderizar();

    await escolherEBaixar();

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível baixar o modelo.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });

  it("cancelar o download avisa que o progresso fica guardado", async () => {
    const download = promessaControlavel<string>();
    simular({
      catalogo_de_modelos: catalogo,
      avaliar_espaco_do_modelo: suficiente,
      baixar_modelo: () => download.promessa,
      cancelar_download_modelo: undefined,
    });
    renderizar();
    await escolherEBaixar();

    await clicar(botao("Cancelar"));
    expect(invoke).toHaveBeenCalledWith("cancelar_download_modelo");
    await act(async () => download.resolver("cancelado"));

    expect(screen.getByText(/Download cancelado/)).toBeVisible();
    expect(botao("Continuar o download")).toBeVisible();
  });

  it("se não conseguir marcar o onboarding como concluído, mostra mensagem amigável", async () => {
    simular({
      concluir_onboarding: () =>
        Promise.reject("Não foi possível salvar. Tente de novo."),
    });
    const { aoConcluir } = renderizar({
      ollama: ollama({ modelosInstalados: 2 }),
    });
    await clicar(botao("Continuar"));
    await clicar(botao("Continuar"));

    await clicar(botao("Começar"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível salvar.",
    );
    expect(aoConcluir).not.toHaveBeenCalled();
  });
});
