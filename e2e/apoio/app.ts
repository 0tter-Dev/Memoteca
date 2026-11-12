import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { createServer } from "node:net";
import { join } from "node:path";
import { type Browser, type Page, chromium } from "@playwright/test";
import { type Ambiente, EXE_DO_APP, EXE_DO_OLLAMA_FALSO } from "./ambiente";
import { esperarAte } from "./ollama-falso";

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

/** O processo ainda existe? */
function existe(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** Roda um comando de PowerShell com as funções de janela do Windows (`user32`). */
function user32(comando: string): string {
  const script = [
    'Add-Type -Namespace E2E -Name W -MemberDefinition @"',
    '[DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h, int c);',
    '[DllImport("user32.dll")] public static extern bool IsIconic(System.IntPtr h);',
    '"@',
    comando,
  ].join("\n");
  const saida = spawnSync("powershell", ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true,
  });
  return saida.stdout.trim();
}

/** A versão do WebView2 instalada, lida do registro do Windows. */
function versaoDoWebView2(): string {
  const chave =
    "HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
  const saida = spawnSync("reg", ["query", chave, "/v", "pv"], {
    encoding: "utf8",
    windowsHide: true,
  });
  return saida.status === 0 ? saida.stdout.trim() : "não encontrado";
}

export interface OpcoesDeInicio {
  /** Endereço da "release" do Ollama que o instalador falso consulta. */
  urlDaRelease?: string;
}

/**
 * O Memoteca de verdade (o `memoteca.exe` do build `e2e`) com a janela controlada pelo
 * Playwright por CDP. Cada instância tem o próprio ambiente, e `reiniciar` encerra o processo
 * e inicia outro, então "fechar e reabrir" é um reinício de verdade, e não só recarregar a tela.
 */
export class AppEmTeste {
  private processo: ChildProcess | null = null;
  private navegador: Browser | null = null;
  private _pagina: Page | null = null;
  private janela = "0";

  constructor(private readonly ambiente: Ambiente) {}

  get pagina(): Page {
    if (!this._pagina) {
      throw new Error("O app não está aberto.");
    }
    return this._pagina;
  }

  get pid(): number {
    if (!this.processo?.pid) {
      throw new Error("O app não está aberto.");
    }
    return this.processo.pid;
  }

  get aberto(): boolean {
    return this.processo?.pid !== undefined && existe(this.processo.pid);
  }

  /** Onde o teste deixa o arquivo de backup que o app vai "escolher" ao importar. */
  get arquivoParaImportar(): string {
    return join(this.ambiente.escolhas, "importar.json");
  }

  /** Conteúdo do log do app, para conferir que um erro técnico foi para lá. */
  log(): string {
    const caminho = join(this.ambiente.dados, "logs", "memoteca.log");
    return existsSync(caminho) ? readFileSync(caminho, "utf8") : "";
  }

  async iniciar(opcoes: OpcoesDeInicio = {}): Promise<void> {
    const porta = await portaLivre();
    const env = this.ambienteDoProcesso(porta, opcoes);
    const saida: string[] = [];
    let saiu: string | null = null;
    const processo = spawn(EXE_DO_APP, [], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: false,
    });
    this.processo = processo;
    for (const fluxo of [processo.stdout, processo.stderr]) {
      fluxo.on("data", (dados: Buffer) => {
        saida.push(dados.toString("utf8"));
        saida.splice(0, Math.max(0, saida.length - 50));
      });
    }
    processo.once("error", (erro) => {
      saiu = `não foi possível iniciar: ${erro.message}`;
    });
    processo.once("exit", (codigo, sinal) => {
      saiu = `saiu com código ${codigo ?? sinal}`;
    });

    try {
      await esperarAte(
        async () => {
          if (saiu !== null) {
            return true;
          }
          try {
            const resposta = await fetch(
              `http://127.0.0.1:${porta}/json/version`,
            );
            return resposta.ok;
          } catch {
            return false;
          }
        },
        30_000,
        "o app abrir a janela",
      );
    } catch (erro) {
      throw this.falhaAoIniciar(erro, saiu, saida.join(""));
    }
    if (saiu !== null) {
      throw this.falhaAoIniciar(
        new Error("O app encerrou ao iniciar"),
        saiu,
        saida.join(""),
      );
    }

    this.navegador = await chromium.connectOverCDP(`http://127.0.0.1:${porta}`);
    const contexto = this.navegador.contexts()[0];
    let pagina = contexto.pages()[0];
    if (!pagina) {
      pagina = await contexto.waitForEvent("page");
    }
    await pagina.waitForLoadState("domcontentloaded");
    this._pagina = pagina;
  }

  /** Junta o que ajuda a entender por que o app não abriu (sobretudo no CI, onde não se vê). */
  private falhaAoIniciar(
    erro: unknown,
    saiu: string | null,
    saida: string,
  ): Error {
    const pasta = (caminho: string): string =>
      existsSync(caminho) ? readdirSync(caminho).join(", ") : "(não existe)";
    const linhas = [
      erro instanceof Error ? erro.message : String(erro),
      `processo: ${saiu ?? "ainda rodando"}`,
      `saída do app: ${saida.trim() || "(vazia)"}`,
      `pasta de dados: ${pasta(this.ambiente.dados)}`,
      `log do app: ${this.log().trim() || "(vazio)"}`,
      `perfil do WebView2: ${pasta(this.ambiente.webview)}`,
      `WebView2 instalado: ${versaoDoWebView2()}`,
    ];
    return new Error(linhas.join("\n"));
  }

  /** O ambiente em que o app roda: isolado do usuário e sem o Ollama de verdade. */
  private ambienteDoProcesso(
    porta: number,
    opcoes: OpcoesDeInicio,
  ): NodeJS.ProcessEnv {
    const sistema = process.env.SystemRoot ?? "C:\\Windows";
    return {
      SystemRoot: sistema,
      windir: sistema,
      USERPROFILE: process.env.USERPROFILE,
      APPDATA: process.env.APPDATA,
      ProgramData: process.env.ProgramData,
      ProgramFiles: process.env.ProgramFiles,
      "ProgramFiles(x86)": process.env["ProgramFiles(x86)"],
      // Sem o Ollama de verdade no PATH: o único Ollama que existe é o falso.
      PATH: `${sistema}\\System32;${sistema}`,
      LOCALAPPDATA: this.ambiente.localAppData,
      TEMP: this.ambiente.temporaria,
      TMP: this.ambiente.temporaria,
      OLLAMA_MODELS: this.ambiente.modelos,
      WEBVIEW2_USER_DATA_FOLDER: this.ambiente.webview,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${porta}`,
      MEMOTECA_E2E_DADOS: this.ambiente.dados,
      MEMOTECA_E2E_ESCOLHAS: this.ambiente.escolhas,
      MEMOTECA_E2E_OLLAMA_FALSO: EXE_DO_OLLAMA_FALSO,
      MEMOTECA_E2E_RELEASE_URL:
        opcoes.urlDaRelease ?? "http://127.0.0.1:9/release",
    };
  }

  /**
   * Abre o executável uma segunda vez, como o usuário que clica no atalho de novo. Devolve o
   * código de saída da segunda execução, ou `null` se ela não terminou no prazo.
   */
  async abrirSegundaInstancia(): Promise<number | null> {
    const porta = await portaLivre();
    const segundo = spawn(EXE_DO_APP, [], {
      env: this.ambienteDoProcesso(porta, {}),
      stdio: "ignore",
    });
    const codigo = await Promise.race([
      new Promise<number | null>((resolver) =>
        segundo.once("exit", (saida) => resolver(saida)),
      ),
      new Promise<null>((resolver) => setTimeout(() => resolver(null), 20_000)),
    ]);
    if (codigo === null) {
      segundo.kill();
    }
    return codigo;
  }

  /** Quantos processos `memoteca.exe` existem agora. */
  quantosProcessosDoApp(): number {
    const saida = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        "(Get-Process -Name memoteca -ErrorAction SilentlyContinue | Measure-Object).Count",
      ],
      { encoding: "utf8", windowsHide: true },
    );
    return Number.parseInt(saida.stdout.trim(), 10);
  }

  /**
   * Minimiza a janela do app (como o usuário que a deixou na barra de tarefas). Guarda o
   * identificador da janela, porque o Windows deixa de informá-lo depois de minimizada.
   */
  minimizarJanela(): void {
    this.janela = user32(`(Get-Process -Id ${this.pid}).MainWindowHandle`);
    user32(`[void][E2E.W]::ShowWindow([System.IntPtr]${this.janela}, 6)`);
  }

  /** A janela que `minimizarJanela` minimizou continua minimizada? */
  janelaMinimizada(): boolean {
    return (
      user32(`[E2E.W]::IsIconic([System.IntPtr]${this.janela})`) === "True"
    );
  }

  /** Fecha como o usuário fecha: pede à janela que feche e espera o app encerrar. */
  async fecharComoOUsuario(): Promise<void> {
    const pid = this.pid;
    spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        `(Get-Process -Id ${pid} -ErrorAction SilentlyContinue).CloseMainWindow() | Out-Null`,
      ],
      { windowsHide: true },
    );
    await esperarAte(() => !existe(pid), 20_000, "o app fechar");
    await this.soltar();
  }

  /** Encerra o processo à força, como uma queda. */
  async matar(): Promise<void> {
    const pid = this.processo?.pid;
    if (pid && existe(pid)) {
      spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], {
        windowsHide: true,
      });
      await esperarAte(() => !existe(pid), 10_000, "o app encerrar");
    }
    await this.soltar();
  }

  /** Fecha o app e abre outro processo, com os mesmos dados. Devolve os dois `pid`. */
  async reiniciar(
    opcoes: OpcoesDeInicio = {},
  ): Promise<{ antes: number; depois: number }> {
    const antes = this.pid;
    await this.fecharComoOUsuario();
    await this.iniciar(opcoes);
    return { antes, depois: this.pid };
  }

  private async soltar(): Promise<void> {
    await this.navegador?.close().catch(() => undefined);
    this.navegador = null;
    this._pagina = null;
    this.processo = null;
  }
}
