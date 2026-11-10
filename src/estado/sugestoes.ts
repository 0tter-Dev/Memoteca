import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { EscopoDeMemoria, ResultadoDaSugestao } from "../tipos";
import { useMemorias } from "./memorias";

const MENSAGEM_PADRAO =
  "Não foi possível concluir. Feche o aplicativo, abra-o de novo e tente outra vez.";

function mensagemDoErro(erro: unknown): string {
  return typeof erro === "string" ? erro : MENSAGEM_PADRAO;
}

/** Uma sugestão na tela, com o escopo que o usuário escolheu para ela. */
export interface Sugestao {
  id: number;
  texto: string;
  escopo: EscopoDeMemoria;
  /** Está sendo criada (aprovada) neste momento. */
  salvando: boolean;
  /** Mensagem amigável se a aprovação falhou. A sugestão continua na lista. */
  erro: string | null;
}

interface EstadoDeSugestoes {
  /** O diálogo de sugestões está aberto. */
  aberta: boolean;
  /** Esperando o modelo responder. */
  gerando: boolean;
  /** O modelo respondeu (mesmo que sem nenhuma sugestão). */
  concluiu: boolean;
  /** Mensagem amigável quando o pedido falhou. */
  erro: string | null;
  sugestoes: Sugestao[];
  /** Quantas sugestões foram aprovadas nesta abertura. */
  aprovadas: number;
  conversaId: string | null;

  /** Abre o diálogo e pede as sugestões ao modelo da conversa. Não cria nenhuma memória. */
  abrir: (conversaId: string) => Promise<void>;
  tentarDeNovo: () => Promise<void>;
  /** Fecha o diálogo. Se o modelo ainda está respondendo, cancela. Nada é criado. */
  fechar: () => void;
  mudarEscopo: (id: number, escopo: EscopoDeMemoria) => void;
  /** Aprova uma sugestão: cria **exatamente uma** memória no escopo escolhido. */
  aprovar: (id: number, projetoId: string) => Promise<void>;
  /** Rejeita uma sugestão: some da lista e nada é criado. */
  rejeitar: (id: number) => void;
}

const estadoInicial = {
  aberta: false,
  gerando: false,
  concluiu: false,
  erro: null,
  sugestoes: [] as Sugestao[],
  aprovadas: 0,
  conversaId: null,
};

export const useSugestoes = create<EstadoDeSugestoes>()((set, get) => {
  /** Cada pedido tem um número. Uma resposta de um pedido antigo (já cancelado) é descartada. */
  let rodada = 0;
  let proximoId = 1;

  async function pedir(conversaId: string) {
    const minha = ++rodada;
    set({
      aberta: true,
      gerando: true,
      concluiu: false,
      erro: null,
      sugestoes: [],
      aprovadas: 0,
      conversaId,
    });
    try {
      const resultado = await invoke<ResultadoDaSugestao>("sugerir_memorias", {
        conversaId,
      });
      if (minha !== rodada) {
        return;
      }
      if (resultado.cancelada) {
        set({ gerando: false });
        return;
      }
      set({
        gerando: false,
        concluiu: true,
        sugestoes: resultado.sugestoes.map((s) => ({
          id: proximoId++,
          texto: s.texto,
          escopo: "projeto",
          salvando: false,
          erro: null,
        })),
      });
    } catch (erro: unknown) {
      if (minha === rodada) {
        set({ gerando: false, erro: mensagemDoErro(erro) });
      }
    }
  }

  return {
    ...estadoInicial,

    abrir: (conversaId) => pedir(conversaId),

    tentarDeNovo: async () => {
      const { conversaId, gerando } = get();
      if (conversaId && !gerando) {
        await pedir(conversaId);
      }
    },

    fechar: () => {
      const { gerando } = get();
      rodada += 1;
      set({ ...estadoInicial });
      if (gerando) {
        // Fecha a conexão com o Ollama. Se a resposta já tinha chegado, não há o que cancelar.
        void Promise.resolve(invoke("cancelar_geracao")).catch(() => undefined);
      }
    },

    mudarEscopo: (id, escopo) =>
      set((estado) => ({
        sugestoes: estado.sugestoes.map((s) =>
          s.id === id ? { ...s, escopo } : s,
        ),
      })),

    aprovar: async (id, projetoId) => {
      const alvo = get().sugestoes.find((s) => s.id === id);
      if (!alvo || alvo.salvando) {
        return;
      }
      const minha = rodada;
      const atualizar = (parcial: Partial<Sugestao>) =>
        set((estado) => ({
          sugestoes: estado.sugestoes.map((s) =>
            s.id === id ? { ...s, ...parcial } : s,
          ),
        }));
      atualizar({ salvando: true, erro: null });

      const falha = await useMemorias
        .getState()
        .criar(alvo.escopo, projetoId, alvo.texto);
      if (minha !== rodada) {
        return;
      }
      if (falha) {
        atualizar({ salvando: false, erro: falha });
        return;
      }
      set((estado) => ({
        sugestoes: estado.sugestoes.filter((s) => s.id !== id),
        aprovadas: estado.aprovadas + 1,
      }));
    },

    rejeitar: (id) =>
      set((estado) => ({
        sugestoes: estado.sugestoes.filter((s) => s.id !== id),
      })),
  };
});

/** Volta o estado ao inicial. Usado pelos testes. */
export function reiniciarSugestoes() {
  useSugestoes.setState(estadoInicial);
}
