import { existsSync } from "node:fs";
import { EXE_DO_APP, EXE_DO_OLLAMA_FALSO, PORTA_DO_OLLAMA } from "./ambiente";
import { alguemRespondeNaPorta } from "./ollama-falso";

/**
 * Roda uma vez antes de todos os testes. O E2E ocupa a porta do Ollama com um servidor falso,
 * então se recusa a rodar onde já há um Ollama de verdade, para não interferir nele.
 */
export default async function preparo(): Promise<void> {
  for (const executavel of [EXE_DO_APP, EXE_DO_OLLAMA_FALSO]) {
    if (!existsSync(executavel)) {
      throw new Error(
        `Não encontrei ${executavel}. Rode "pnpm e2e:build" antes do E2E.`,
      );
    }
  }
  if (await alguemRespondeNaPorta(PORTA_DO_OLLAMA)) {
    throw new Error(
      `Há algo respondendo na porta ${PORTA_DO_OLLAMA}, provavelmente o Ollama de verdade. ` +
        "O E2E não roda assim, para não interferir nele. Feche o Ollama e tente de novo.",
    );
  }
}
