import { test as base, expect } from "@playwright/test";
import { AppEmTeste } from "./app";
import { type Ambiente, apagarAmbiente, criarAmbiente } from "./ambiente";
import {
  ControleDoFalso,
  OllamaFalsoExterno,
  alguemRespondeNaPorta,
} from "./ollama-falso";
import { createServer } from "node:net";

/** Quem sobe os servidores falsos de um teste. Todos são encerrados ao final. */
export class Falsos {
  private iniciados: OllamaFalsoExterno[] = [];

  /** O "Ollama que já estava rodando": servidor falso na porta `11434`. */
  async ollamaExterno(modelos: string[] = []): Promise<ControleDoFalso> {
    const servidor = new OllamaFalsoExterno(undefined, modelos);
    this.iniciados.push(servidor);
    await servidor.iniciar();
    return new ControleDoFalso(servidor.url);
  }

  /** O servidor da "release" do Ollama, numa porta livre. Devolve o endereço da release. */
  async release(): Promise<string> {
    const porta = await portaLivre();
    const servidor = new OllamaFalsoExterno(porta);
    this.iniciados.push(servidor);
    await servidor.iniciar();
    return `${servidor.url}/release`;
  }

  async encerrar(): Promise<void> {
    for (const servidor of this.iniciados.reverse()) {
      await servidor.parar();
    }
    this.iniciados = [];
  }
}

async function portaLivre(): Promise<number> {
  return new Promise((resolver, rejeitar) => {
    const servidor = createServer();
    servidor.once("error", rejeitar);
    servidor.listen(0, "127.0.0.1", () => {
      const endereco = servidor.address();
      const porta =
        typeof endereco === "object" && endereco ? endereco.port : 0;
      servidor.close(() => resolver(porta));
    });
  });
}

interface Fixtures {
  ambiente: Ambiente;
  app: AppEmTeste;
  falsos: Falsos;
}

export const test = base.extend<Fixtures>({
  // eslint-disable-next-line no-empty-pattern
  ambiente: async ({}, usar) => {
    const ambiente = criarAmbiente();
    await usar(ambiente);
    apagarAmbiente(ambiente);
  },

  // eslint-disable-next-line no-empty-pattern
  falsos: async ({}, usar) => {
    if (await alguemRespondeNaPorta(11434)) {
      throw new Error(
        "Já há um Ollama respondendo na porta 11434. Feche-o antes de rodar o E2E.",
      );
    }
    const falsos = new Falsos();
    await usar(falsos);
    await falsos.encerrar();
  },

  app: async ({ ambiente, falsos }, usar) => {
    // `falsos` é pedido aqui para o app ser encerrado antes dos servidores.
    void falsos;
    const app = new AppEmTeste(ambiente);
    await usar(app);
    await app.matar();
  },
});

export { expect };
