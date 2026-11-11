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
import { reiniciarSugestoes } from "./estado/sugestoes";
import { descreverItens, totalDe } from "./itensDeBackup";
import TelaPrincipal from "./TelaPrincipal";
import type {
  AnaliseDoBackup,
  Contagem,
  EstadoOllama,
  Projeto,
  ResultadoDaExportacao,
  ResumoDaImportacao,
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
  modeloPadrao: null,
  criadoEm: "",
  atualizadoEm: "",
};

function contagem(novos: number, jaExistem = 0, ignorados = 0): Contagem {
  return { novos, jaExistem, ignorados };
}

function resumo(parcial: Partial<ResumoDaImportacao> = {}): ResumoDaImportacao {
  return {
    projetos: contagem(1),
    conversas: contagem(2),
    mensagens: contagem(10),
    memorias: contagem(3),
    ...parcial,
  };
}

function analise(r: ResumoDaImportacao = resumo()): AnaliseDoBackup {
  return {
    caminho: "C:\\Backups\\memoteca-backup-2026-11-10.json",
    exportadoEm: "2026-11-10T03:14:40.123Z",
    resumo: r,
  };
}

interface Opcoes {
  exportar?: () => Promise<ResultadoDaExportacao>;
  analisar?: () => Promise<AnaliseDoBackup | null>;
  importar?: () => Promise<ResumoDaImportacao>;
}

function simular(opcoes: Opcoes = {}) {
  invoke.mockImplementation(async (comando: string) => {
    switch (comando) {
      case "listar_projetos":
        return [projeto];
      case "listar_conversas":
        return [];
      case "exportar_backup":
        return (
          opcoes.exportar ??
          (async () => ({ cancelado: true, caminho: null, resumo: null }))
        )();
      case "analisar_backup":
        return (opcoes.analisar ?? (async () => analise()))();
      case "importar_backup_analisado":
        return (opcoes.importar ?? (async () => resumo()))();
      case "descartar_backup_analisado":
        return undefined;
      default:
        throw `comando inesperado: ${comando}`;
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

async function abrirConfiguracoes() {
  render(<TelaPrincipal ollama={ollama} erroDeDados={null} />);
  await screen.findByRole("heading", { level: 2, name: "Projeto A" });
  await clicar(botao("⚙ Configurações"));
  await screen.findByRole("region", { name: "Configurações" });
}

beforeEach(() => {
  invoke.mockReset();
  reiniciarProjetos();
  reiniciarConversas();
  reiniciarMemorias();
  reiniciarSugestoes();
});

describe("Configurações: Dados", () => {
  it("abre pela lateral, mostra a seção Dados e dá para voltar", async () => {
    simular();
    await abrirConfiguracoes();

    expect(screen.getByRole("heading", { name: "Dados" })).toBeVisible();
    expect(botao("Exportar backup…")).toBeEnabled();
    expect(botao("Importar backup…")).toBeEnabled();
    expect(botao("⚙ Configurações")).toHaveAttribute("aria-current", "true");

    await clicar(botao("Voltar"));
    expect(
      await screen.findByRole("heading", { level: 2, name: "Projeto A" }),
    ).toBeVisible();
  });

  it("avisa que o arquivo não é criptografado e que importar não apaga nada", async () => {
    simular();
    await abrirConfiguracoes();

    expect(screen.getByText(/não é criptografado/)).toBeVisible();
    expect(
      screen.getByText(/nunca apaga nem altera o que você já tem/),
    ).toBeVisible();
  });
});

describe("Exportar", () => {
  it("mostra onde o backup foi salvo e quantos itens foram", async () => {
    simular({
      exportar: async () => ({
        cancelado: false,
        caminho: "C:\\Backups\\memoteca-backup-2026-11-10.json",
        resumo: { projetos: 1, conversas: 2, mensagens: 10, memorias: 1 },
      }),
    });
    await abrirConfiguracoes();

    await clicar(botao("Exportar backup…"));

    const aviso = await screen.findByText(/Backup salvo em/);
    expect(aviso).toHaveTextContent(
      "Backup salvo em C:\\Backups\\memoteca-backup-2026-11-10.json: 1 projeto, 2 conversas, 10 mensagens e 1 memória.",
    );
    expect(chamadas("exportar_backup")).toHaveLength(1);
  });

  it("se o usuário fecha a janela sem escolher, não mostra nada", async () => {
    simular();
    await abrirConfiguracoes();

    await clicar(botao("Exportar backup…"));

    expect(chamadas("exportar_backup")).toHaveLength(1);
    expect(screen.queryByText(/Backup salvo/)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("falha na gravação mostra a mensagem amigável, nunca o erro cru", async () => {
    simular({
      exportar: async () => {
        throw "O Memoteca não tem permissão para gravar nesse local. Escolha outra pasta, como Documentos. Nenhum arquivo foi criado.";
      },
    });
    await abrirConfiguracoes();

    await clicar(botao("Exportar backup…"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Nenhum arquivo foi criado",
    );
  });

  it("erro que não é texto vira a mensagem padrão", async () => {
    simular({
      exportar: async () => {
        throw new TypeError("Cannot read properties of undefined");
      },
    });
    await abrirConfiguracoes();

    await clicar(botao("Exportar backup…"));

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Não foi possível concluir.");
    expect(alerta).not.toHaveTextContent("Cannot read");
  });
});

describe("Importar", () => {
  it("mostra o que o backup faria, antes de gravar qualquer coisa", async () => {
    simular({
      analisar: async () =>
        analise(
          resumo({
            projetos: contagem(1, 1, 1),
            conversas: contagem(2, 3, 0),
            mensagens: contagem(10, 20, 4),
            memorias: contagem(3, 0, 1),
          }),
        ),
    });
    await abrirConfiguracoes();

    await clicar(botao("Importar backup…"));

    const dialogo = await screen.findByRole("alertdialog");
    expect(dialogo).toHaveTextContent("Importar este backup?");
    expect(dialogo).toHaveTextContent(
      "Vai adicionar: 1 projeto, 2 conversas, 10 mensagens e 3 memórias.",
    );
    expect(dialogo).toHaveTextContent(
      "Já existem aqui e ficam como estão: 1 projeto, 3 conversas e 20 mensagens.",
    );
    expect(dialogo).toHaveTextContent(
      "Foram excluídos aqui e não voltam: 1 projeto, 4 mensagens e 1 memória.",
    );
    expect(dialogo).toHaveTextContent("Nada do que você já tem é apagado");
    expect(chamadas("importar_backup_analisado")).toHaveLength(0);
  });

  it("confirmar importa, avisa o que entrou e atualiza a lista de projetos", async () => {
    simular({ importar: async () => resumo() });
    await abrirConfiguracoes();
    await clicar(botao("Importar backup…"));
    const dialogo = await screen.findByRole("alertdialog");
    const projetosAntes = chamadas("listar_projetos").length;

    await clicar(within(dialogo).getByRole("button", { name: "Importar" }));

    await waitFor(() =>
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
    );
    expect(chamadas("importar_backup_analisado")).toHaveLength(1);
    expect(
      await screen.findByText(
        "Backup importado: 1 projeto, 2 conversas, 10 mensagens e 3 memórias adicionados.",
      ),
    ).toBeVisible();
    expect(chamadas("listar_projetos").length).toBeGreaterThan(projetosAntes);
  });

  it("cancelar na confirmação não importa nada e descarta o backup analisado", async () => {
    simular();
    await abrirConfiguracoes();
    await clicar(botao("Importar backup…"));
    const dialogo = await screen.findByRole("alertdialog");

    await clicar(within(dialogo).getByRole("button", { name: "Cancelar" }));

    await waitFor(() =>
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
    );
    expect(chamadas("importar_backup_analisado")).toHaveLength(0);
    expect(chamadas("descartar_backup_analisado")).toHaveLength(1);
    expect(screen.queryByText(/Backup importado/)).not.toBeInTheDocument();
  });

  it("se o usuário fecha a janela de arquivo sem escolher, nada acontece", async () => {
    simular({ analisar: async () => null });
    await abrirConfiguracoes();

    await clicar(botao("Importar backup…"));

    expect(chamadas("analisar_backup")).toHaveLength(1);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(chamadas("importar_backup_analisado")).toHaveLength(0);
  });

  it("arquivo inválido, truncado ou de versão desconhecida é recusado com mensagem amigável e nada é importado (falha)", async () => {
    simular({
      analisar: async () => {
        throw "Este arquivo não é um backup do Memoteca, ou está incompleto ou danificado. Nada foi importado nem alterado. Escolha outro arquivo.";
      },
    });
    await abrirConfiguracoes();

    await clicar(botao("Importar backup…"));

    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Nada foi importado nem alterado");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(chamadas("importar_backup_analisado")).toHaveLength(0);
    expect(botao("Importar backup…")).toBeEnabled();
  });

  it("se a gravação falha, o diálogo continua com a mensagem, e nada aparece como importado (falha)", async () => {
    simular({
      importar: async () => {
        throw "Não foi possível salvar seus dados agora. Nada foi importado pela metade.";
      },
    });
    await abrirConfiguracoes();
    await clicar(botao("Importar backup…"));
    const dialogo = await screen.findByRole("alertdialog");

    await clicar(within(dialogo).getByRole("button", { name: "Importar" }));

    expect(await within(dialogo).findByRole("alert")).toHaveTextContent(
      "Nada foi importado pela metade",
    );
    expect(screen.queryByText(/Backup importado/)).not.toBeInTheDocument();
  });

  it("sem nada novo, não oferece importar e só deixa fechar", async () => {
    simular({
      analisar: async () =>
        analise({
          projetos: contagem(0, 1, 0),
          conversas: contagem(0, 2, 0),
          mensagens: contagem(0, 8, 0),
          memorias: contagem(0, 0, 1),
        }),
    });
    await abrirConfiguracoes();

    await clicar(botao("Importar backup…"));

    const dialogo = await screen.findByRole("alertdialog");
    expect(dialogo).toHaveTextContent("Não há nada novo para importar");
    expect(
      within(dialogo).queryByRole("button", { name: "Importar" }),
    ).not.toBeInTheDocument();
    await clicar(within(dialogo).getByRole("button", { name: "Fechar" }));
    expect(chamadas("importar_backup_analisado")).toHaveLength(0);
  });

  it("enquanto a janela de arquivo está aberta, os botões ficam desabilitados", async () => {
    let escolher!: (valor: AnaliseDoBackup | null) => void;
    simular({ analisar: () => new Promise((resolve) => (escolher = resolve)) });
    await abrirConfiguracoes();

    await clicar(botao("Importar backup…"));

    expect(botao("Exportar backup…")).toBeDisabled();
    expect(botao("Importar backup…")).toBeDisabled();
    await act(async () => escolher(null));
    expect(botao("Exportar backup…")).toBeEnabled();
  });
});

describe("Descrição dos itens", () => {
  it("usa o singular e o plural, junta com vírgulas e 'e', e pula o que está em zero", () => {
    expect(
      descreverItens({ projetos: 1, conversas: 2, mensagens: 10, memorias: 1 }),
    ).toBe("1 projeto, 2 conversas, 10 mensagens e 1 memória");
    expect(
      descreverItens({ projetos: 0, conversas: 1, mensagens: 0, memorias: 0 }),
    ).toBe("1 conversa");
    expect(
      descreverItens({ projetos: 0, conversas: 0, mensagens: 3, memorias: 2 }),
    ).toBe("3 mensagens e 2 memórias");
    expect(
      descreverItens({ projetos: 0, conversas: 0, mensagens: 0, memorias: 0 }),
    ).toBe("nenhum item");
    expect(
      totalDe({ projetos: 1, conversas: 2, mensagens: 3, memorias: 4 }),
    ).toBe(10);
  });
});
