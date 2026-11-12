import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/** Raiz do repositório. */
export const RAIZ = resolve(import.meta.dirname, "..", "..");

/** O app compilado com o recurso `e2e` (`pnpm e2e:build`). */
export const EXE_DO_APP = join(
  RAIZ,
  "src-tauri",
  "target",
  "debug",
  "memoteca.exe",
);

/** O servidor Ollama falso (`cargo build --example ollama_falso`). */
export const EXE_DO_OLLAMA_FALSO = join(
  RAIZ,
  "src-tauri",
  "target",
  "debug",
  "examples",
  "ollama_falso.exe",
);

/** A porta fixa do Ollama, que o app usa. */
export const PORTA_DO_OLLAMA = 11434;

/**
 * Pastas de um teste. Cada teste tem as suas, então nada do usuário é tocado e um teste não
 * enxerga o que o outro deixou.
 */
export interface Ambiente {
  /** Pasta-mãe, apagada ao final. */
  raiz: string;
  /** Dados do app: banco, cópias de segurança e log (`MEMOTECA_E2E_DADOS`). */
  dados: string;
  /** Onde o teste "escolhe" arquivos de backup (`MEMOTECA_E2E_ESCOLHAS`). */
  escolhas: string;
  /** `LOCALAPPDATA` do app: o Ollama falso é "instalado" em `Programs\Ollama` aqui. */
  localAppData: string;
  /** Pasta dos modelos (`OLLAMA_MODELS`). */
  modelos: string;
  /** Perfil do WebView2. */
  webview: string;
  /** Pasta temporária do app (`TEMP`). */
  temporaria: string;
}

export function criarAmbiente(): Ambiente {
  const raiz = mkdtempSync(join(tmpdir(), "memoteca-e2e-"));
  const ambiente: Ambiente = {
    raiz,
    dados: join(raiz, "dados"),
    escolhas: join(raiz, "escolhas"),
    localAppData: join(raiz, "local"),
    modelos: join(raiz, "modelos"),
    webview: join(raiz, "webview"),
    temporaria: join(raiz, "temp"),
  };
  for (const pasta of [
    ambiente.dados,
    ambiente.escolhas,
    ambiente.localAppData,
    ambiente.modelos,
    ambiente.webview,
    ambiente.temporaria,
  ]) {
    mkdirSync(pasta, { recursive: true });
  }
  return ambiente;
}

export function apagarAmbiente(ambiente: Ambiente): void {
  // Os processos do WebView2 demoram um pouco para soltar os arquivos depois que o app fecha.
  // Limpar a pasta temporária nunca pode fazer um teste falhar.
  try {
    rmSync(ambiente.raiz, {
      recursive: true,
      force: true,
      maxRetries: 20,
      retryDelay: 250,
    });
  } catch {
    // Fica para o sistema limpar a pasta temporária.
  }
}
