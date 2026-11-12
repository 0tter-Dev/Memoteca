import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { DadosProjeto, Projeto } from "../tipos";
import { mensagemDoErro } from "../erros";

/** O que a área principal mostra: o projeto, o formulário de um novo ou o de edição, ou as memórias. */
export type Modo =
  "ver" | "novo" | "editar" | "memorias" | "configuracoes" | "sobre";

/** Aba aberta na tela de memórias. */
export type AbaDeMemorias = "projeto" | "global";

interface EstadoDeProjetos {
  projetos: Projeto[];
  /** Os projetos já foram lidos do banco (mesmo que a lista esteja vazia). */
  carregado: boolean;
  /** Mensagem amigável quando a lista não carrega. */
  erro: string | null;
  selecionadoId: string | null;
  modo: Modo;
  abaDeMemorias: AbaDeMemorias;

  carregar: () => Promise<void>;
  selecionar: (id: string) => void;
  iniciarNovo: () => void;
  iniciarEdicao: () => void;
  /** Abre a tela de memórias na aba dada. */
  abrirMemorias: (aba: AbaDeMemorias) => void;
  /** Abre a tela de configurações. */
  abrirConfiguracoes: () => void;
  /** Abre a tela "Sobre". */
  abrirSobre: () => void;
  cancelarFormulario: () => void;
  /** Salva (cria ou edita, conforme o modo). Devolve a mensagem de erro, ou `null`. */
  salvar: (dados: DadosProjeto) => Promise<string | null>;
  /** Exclui o projeto. Devolve a mensagem de erro, ou `null`. */
  excluir: (id: string) => Promise<string | null>;
}

const estadoInicial = {
  projetos: [],
  carregado: false,
  erro: null,
  selecionadoId: null,
  modo: "ver" as Modo,
  abaDeMemorias: "projeto" as AbaDeMemorias,
};

export const useProjetos = create<EstadoDeProjetos>()((set, get) => ({
  ...estadoInicial,

  carregar: async () => {
    try {
      const projetos = await invoke<Projeto[]>("listar_projetos");
      const { selecionadoId } = get();
      const existe = projetos.some((p) => p.id === selecionadoId);
      set({
        projetos,
        carregado: true,
        erro: null,
        // Mantém o projeto aberto; senão, abre o primeiro da lista.
        selecionadoId: existe ? selecionadoId : (projetos[0]?.id ?? null),
      });
    } catch (erro: unknown) {
      set({ carregado: true, erro: mensagemDoErro(erro) });
    }
  },

  selecionar: (id) => set({ selecionadoId: id, modo: "ver" }),
  iniciarNovo: () => set({ modo: "novo" }),
  iniciarEdicao: () => set({ modo: "editar" }),
  abrirMemorias: (aba) => set({ modo: "memorias", abaDeMemorias: aba }),
  abrirConfiguracoes: () => set({ modo: "configuracoes" }),
  abrirSobre: () => set({ modo: "sobre" }),
  cancelarFormulario: () => set({ modo: "ver" }),

  salvar: async (dados) => {
    const { modo, selecionadoId } = get();
    try {
      const salvo =
        modo === "editar" && selecionadoId
          ? await invoke<Projeto>("atualizar_projeto", {
              id: selecionadoId,
              dados,
            })
          : await invoke<Projeto>("criar_projeto", { dados });
      await get().carregar();
      set({ selecionadoId: salvo.id, modo: "ver" });
      return null;
    } catch (erro: unknown) {
      return mensagemDoErro(erro);
    }
  },

  excluir: async (id) => {
    try {
      await invoke("excluir_projeto", { id });
      set({ selecionadoId: null, modo: "ver" });
      await get().carregar();
      return null;
    } catch (erro: unknown) {
      return mensagemDoErro(erro);
    }
  },
}));

/** Volta o estado ao inicial. Usado pelos testes. */
export function reiniciarProjetos() {
  useProjetos.setState(estadoInicial);
}
