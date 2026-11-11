import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { EscopoDeMemoria, Memoria } from "../tipos";
import { mensagemDoErro } from "../erros";

interface EstadoDeMemorias {
  globais: Memoria[];
  /** Memórias de cada projeto, pelo ID do projeto. */
  doProjeto: Record<string, Memoria[]>;
  /** Mensagem amigável quando uma lista não carrega. */
  erro: string | null;

  carregarGlobais: () => Promise<void>;
  carregarDoProjeto: (projetoId: string) => Promise<void>;
  /** Cria uma memória. Devolve a mensagem de erro, ou `null`. */
  criar: (
    escopo: EscopoDeMemoria,
    projetoId: string | null,
    conteudo: string,
  ) => Promise<string | null>;
  /** Edita o texto de uma memória. Devolve a mensagem de erro, ou `null`. */
  atualizar: (memoria: Memoria, conteudo: string) => Promise<string | null>;
  /** Remove uma memória, sem aviso nenhum de "contexto mudou". */
  excluir: (memoria: Memoria) => Promise<string | null>;
}

const estadoInicial = {
  globais: [] as Memoria[],
  doProjeto: {} as Record<string, Memoria[]>,
  erro: null,
};

export const useMemorias = create<EstadoDeMemorias>()((set, get) => {
  /** Relê a lista do escopo da memória, que é a fonte de verdade. */
  async function recarregar(escopo: EscopoDeMemoria, projetoId: string | null) {
    if (escopo === "global") {
      await get().carregarGlobais();
    } else if (projetoId) {
      await get().carregarDoProjeto(projetoId);
    }
  }

  return {
    ...estadoInicial,

    carregarGlobais: async () => {
      try {
        const globais = await invoke<Memoria[]>("listar_memorias_globais");
        set({ globais, erro: null });
      } catch (erro: unknown) {
        set({ erro: mensagemDoErro(erro) });
      }
    },

    carregarDoProjeto: async (projetoId) => {
      try {
        const lista = await invoke<Memoria[]>("listar_memorias_do_projeto", {
          projetoId,
        });
        set((estado) => ({
          doProjeto: { ...estado.doProjeto, [projetoId]: lista },
          erro: null,
        }));
      } catch (erro: unknown) {
        set({ erro: mensagemDoErro(erro) });
      }
    },

    criar: async (escopo, projetoId, conteudo) => {
      try {
        await invoke<Memoria>("criar_memoria", {
          escopo,
          projetoId: escopo === "projeto" ? projetoId : null,
          conteudo,
        });
        await recarregar(escopo, projetoId);
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },

    atualizar: async (memoria, conteudo) => {
      try {
        await invoke<Memoria>("atualizar_memoria", {
          id: memoria.id,
          conteudo,
        });
        await recarregar(memoria.escopo, memoria.projetoId);
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },

    excluir: async (memoria) => {
      try {
        await invoke("excluir_memoria", { id: memoria.id });
        await recarregar(memoria.escopo, memoria.projetoId);
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },
  };
});

/** Volta o estado ao inicial. Usado pelos testes. */
export function reiniciarMemorias() {
  useMemorias.setState(estadoInicial);
}
