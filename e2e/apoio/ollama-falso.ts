import { type ChildProcess, spawn } from "node:child_process";
import { createServer } from "node:http";
import { EXE_DO_OLLAMA_FALSO, PORTA_DO_OLLAMA } from "./ambiente";

export interface PedidoRecebido {
  metodo: string;
  caminho: string;
  corpo: string;
}

/** Espera até `condicao` ser verdadeira. */
export async function esperarAte(
  condicao: () => Promise<boolean> | boolean,
  prazoMs: number,
  descricao: string,
): Promise<void> {
  const limite = Date.now() + prazoMs;
  while (Date.now() < limite) {
    if (await condicao()) {
      return;
    }
    await new Promise((resolver) => setTimeout(resolver, 100));
  }
  throw new Error(`Passou do prazo esperando: ${descricao}`);
}

/** `true` se alguém responde HTTP na porta (um Ollama de verdade, por exemplo). */
export async function alguemRespondeNaPorta(porta: number): Promise<boolean> {
  try {
    await fetch(`http://127.0.0.1:${porta}/api/version`, {
      signal: AbortSignal.timeout(1500),
    });
    return true;
  } catch {
    return false;
  }
}

/**
 * O servidor Ollama falso como um processo à parte: o "Ollama que já estava rodando" (externo)
 * ou, em outra porta, o servidor da "release" que o instalador falso consulta.
 */
export class OllamaFalsoExterno {
  private processo: ChildProcess | null = null;

  constructor(
    readonly porta: number = PORTA_DO_OLLAMA,
    private readonly modelos: string[] = [],
  ) {}

  get url(): string {
    return `http://127.0.0.1:${this.porta}`;
  }

  async iniciar(): Promise<void> {
    this.processo = spawn(
      EXE_DO_OLLAMA_FALSO,
      ["--porta", String(this.porta)],
      {
        stdio: "ignore",
        env: { ...process.env, FALSO_MODELOS: this.modelos.join(",") },
        windowsHide: true,
      },
    );
    await esperarAte(
      () => alguemRespondeNaPorta(this.porta),
      10_000,
      `o servidor falso na porta ${this.porta}`,
    );
  }

  async parar(): Promise<void> {
    const processo = this.processo;
    this.processo = null;
    if (processo && processo.exitCode === null) {
      processo.kill();
      await new Promise((resolver) => processo.once("exit", resolver));
    }
  }
}

/** Fala com as rotas de controle de um servidor falso, que esteja onde estiver. */
export class ControleDoFalso {
  constructor(
    private readonly url: string = `http://127.0.0.1:${PORTA_DO_OLLAMA}`,
  ) {}

  async cenario(nome: string): Promise<void> {
    await fetch(`${this.url}/__e2e/cenario`, {
      method: "POST",
      body: JSON.stringify({ cenario: nome }),
    });
  }

  async requisicoes(): Promise<PedidoRecebido[]> {
    const resposta = await fetch(`${this.url}/__e2e/requisicoes`);
    return (await resposta.json()) as PedidoRecebido[];
  }

  /** Corpos (JSON) dos pedidos feitos a um caminho. */
  async corpos(caminho: string): Promise<Record<string, unknown>[]> {
    return (await this.requisicoes())
      .filter((pedido) => pedido.caminho === caminho)
      .map((pedido) => JSON.parse(pedido.corpo) as Record<string, unknown>);
  }
}

/**
 * Um programa que não é o Ollama ocupando a porta `11434` (R02): responde uma página HTML.
 */
export async function ocuparAPortaComOutroPrograma(): Promise<
  () => Promise<void>
> {
  const servidor = createServer((_pedido, resposta) => {
    resposta.writeHead(200, { "Content-Type": "text/html" });
    resposta.end("<html><body>Outro programa</body></html>");
  });
  await new Promise<void>((resolver, rejeitar) => {
    servidor.once("error", rejeitar);
    servidor.listen(PORTA_DO_OLLAMA, "127.0.0.1", resolver);
  });
  return async () => {
    servidor.closeAllConnections();
    await new Promise((resolver) => servidor.close(resolver));
  };
}
