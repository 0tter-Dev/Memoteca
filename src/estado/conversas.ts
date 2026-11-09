import { Channel, invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type {
  Conversa,
  DadosConversa,
  EventoDeGeracao,
  FalhaDaGeracao,
  Mensagem,
  ResultadoDaGeracao,
} from "../tipos";

const MENSAGEM_PADRAO =
  "Não foi possível concluir. Feche o aplicativo, abra-o de novo e tente outra vez.";

function mensagemDoErro(erro: unknown): string {
  return typeof erro === "string" ? erro : MENSAGEM_PADRAO;
}

/** Texto que chegou até agora da resposta em andamento. */
export interface RespostaEmAndamento {
  conteudo: string;
  raciocinio: string;
}

interface EstadoDeConversas {
  conversasPorProjeto: Record<string, Conversa[]>;
  /** A única conversa ativa. */
  ativaId: string | null;
  mensagens: Mensagem[];
  gerando: boolean;
  /** A resposta que está chegando, ou `null` se não há geração. */
  emAndamento: RespostaEmAndamento | null;
  /** Mensagem amigável da última falha de envio, mostrada dentro da conversa. */
  erroDeEnvio: string | null;
  /** A última resposta foi cortada por limite de tamanho. */
  respostaCortada: boolean;

  carregarConversas: (projetoId: string) => Promise<void>;
  /** Cria uma conversa no projeto e a abre. Devolve a mensagem de erro, ou `null`. */
  criarConversa: (projetoId: string) => Promise<string | null>;
  abrirConversa: (id: string) => Promise<void>;
  fecharConversa: () => void;
  atualizarConversa: (
    id: string,
    dados: DadosConversa,
  ) => Promise<string | null>;
  excluirConversa: (id: string) => Promise<string | null>;
  /** Envia uma mensagem. `persistiu` diz se ela foi gravada (se não, o texto deve ficar no campo). */
  enviar: (texto: string) => Promise<{ persistiu: boolean }>;
  /** "Tentar de novo": responde à última mensagem do usuário, que ficou sem resposta. */
  tentarDeNovo: () => Promise<void>;
  regenerar: () => Promise<void>;
  cancelar: () => void;
}

const estadoInicial = {
  conversasPorProjeto: {} as Record<string, Conversa[]>,
  ativaId: null,
  mensagens: [] as Mensagem[],
  gerando: false,
  emAndamento: null,
  erroDeEnvio: null,
  respostaCortada: false,
};

export const useConversas = create<EstadoDeConversas>()((set, get) => {
  async function recarregarMensagens(id: string) {
    try {
      const mensagens = await invoke<Mensagem[]>("listar_mensagens", {
        conversaId: id,
      });
      if (get().ativaId === id) {
        set({ mensagens });
      }
    } catch (erro: unknown) {
      set({ erroDeEnvio: mensagemDoErro(erro) });
    }
  }

  async function recarregarConversas(projetoId: string) {
    try {
      const lista = await invoke<Conversa[]>("listar_conversas", { projetoId });
      set((estado) => ({
        conversasPorProjeto: {
          ...estado.conversasPorProjeto,
          [projetoId]: lista,
        },
      }));
    } catch {
      // A lista da lateral se atualiza na próxima oportunidade.
    }
  }

  function projetoDaAtiva(): string | null {
    const { ativaId, conversasPorProjeto } = get();
    for (const [projetoId, lista] of Object.entries(conversasPorProjeto)) {
      if (lista.some((c) => c.id === ativaId)) {
        return projetoId;
      }
    }
    return null;
  }

  /** Roda um comando de geração, mostrando os pedaços conforme chegam. */
  async function gerar(comando: string, argumentos: Record<string, unknown>) {
    const id = get().ativaId;
    if (!id || get().gerando) {
      return;
    }
    set({
      gerando: true,
      erroDeEnvio: null,
      respostaCortada: false,
      emAndamento: { conteudo: "", raciocinio: "" },
    });

    const canal = new Channel<EventoDeGeracao>();
    canal.onmessage = (evento) => {
      set((estado) => {
        const atual = estado.emAndamento ?? { conteudo: "", raciocinio: "" };
        return {
          emAndamento:
            evento.tipo === "conteudo"
              ? { ...atual, conteudo: atual.conteudo + evento.texto }
              : { ...atual, raciocinio: atual.raciocinio + evento.texto },
        };
      });
    };

    let falha: string | null = null;
    let cortada = false;
    try {
      const resultado = await invoke<ResultadoDaGeracao>(comando, {
        conversaId: id,
        ...argumentos,
        canal,
      });
      cortada = resultado.cortada;
    } catch (erro: unknown) {
      const tipada = erro as Partial<FalhaDaGeracao> | null;
      falha =
        typeof tipada?.mensagem === "string"
          ? tipada.mensagem
          : MENSAGEM_PADRAO;
    }

    await recarregarMensagens(id);
    const projetoId = projetoDaAtiva();
    if (projetoId) {
      await recarregarConversas(projetoId);
    }
    set({
      gerando: false,
      emAndamento: null,
      erroDeEnvio: falha,
      respostaCortada: cortada,
    });
  }

  return {
    ...estadoInicial,

    carregarConversas: (projetoId) => recarregarConversas(projetoId),

    criarConversa: async (projetoId) => {
      try {
        const conversa = await invoke<Conversa>("criar_conversa", {
          projetoId,
        });
        await recarregarConversas(projetoId);
        set({
          ativaId: conversa.id,
          mensagens: [],
          erroDeEnvio: null,
          respostaCortada: false,
        });
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },

    abrirConversa: async (id) => {
      if (get().gerando) {
        return;
      }
      set({
        ativaId: id,
        mensagens: [],
        erroDeEnvio: null,
        respostaCortada: false,
      });
      await recarregarMensagens(id);
    },

    fecharConversa: () =>
      set({
        ativaId: null,
        mensagens: [],
        erroDeEnvio: null,
        respostaCortada: false,
      }),

    atualizarConversa: async (id, dados) => {
      try {
        const salva = await invoke<Conversa>("atualizar_conversa", {
          id,
          dados,
        });
        await recarregarConversas(salva.projetoId);
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },

    excluirConversa: async (id) => {
      const projetoId = projetoDaAtiva();
      try {
        await invoke("excluir_conversa", { id });
        set({ ativaId: null, mensagens: [], erroDeEnvio: null });
        if (projetoId) {
          await recarregarConversas(projetoId);
        }
        return null;
      } catch (erro: unknown) {
        return mensagemDoErro(erro);
      }
    },

    enviar: async (texto) => {
      const id = get().ativaId;
      if (!id || get().gerando) {
        return { persistiu: false };
      }
      const antes = get().mensagens.length;
      // Mostra a mensagem do usuário já, antes de a geração terminar.
      set((estado) => ({
        mensagens: [
          ...estado.mensagens,
          {
            id: "pendente",
            conversaId: id,
            posicao: antes + 1,
            papel: "user",
            conteudo: texto.trim(),
            raciocinio: null,
            estado: "completa",
            criadoEm: "",
            atualizadoEm: "",
          },
        ],
      }));
      await gerar("enviar_mensagem", { texto });
      return { persistiu: get().mensagens.length > antes };
    },

    tentarDeNovo: () => gerar("gerar_resposta_pendente", {}),
    regenerar: () => gerar("regenerar_resposta", {}),

    cancelar: () => {
      invoke("cancelar_geracao").catch(() => undefined);
    },
  };
});

/** Volta o estado ao inicial. Usado pelos testes. */
export function reiniciarConversas() {
  useConversas.setState(estadoInicial);
}
