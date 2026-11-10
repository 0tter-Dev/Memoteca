/** Estado do Ollama, como o backend devolve (comando `preparar_ollama`). */
export interface EstadoOllama {
  estado: "em_execucao" | "nao_instalado" | "porta_ocupada" | "indisponivel";
  versao: string | null;
  origem: "externo" | "iniciado_pelo_app" | null;
  modelosInstalados: number | null;
  aviso: string | null;
  mensagem: string | null;
}

/** Item do catálogo de modelos (comando `catalogo_de_modelos`). */
export interface ModeloDoCatalogo {
  tag: string;
  nome: string;
  descricao: string;
  faixa: "leve" | "equilibrado" | "potente";
  tamanhoEmBytes: number;
  ramEstimadaEmGb: number;
  raciocinio: boolean;
}

export interface Catalogo {
  versao: number;
  atualizadoEm: string;
  modelos: ModeloDoCatalogo[];
}

/** Avaliação do espaço em disco (comando `avaliar_espaco_do_modelo`). */
export interface AvaliacaoDeEspaco {
  estado: "suficiente" | "alerta" | "insuficiente" | "desconhecido";
  livreEmBytes: number | null;
  necessarioEmBytes: number | null;
  mensagem: string | null;
}

/** Progresso do download de um modelo (canal do comando `baixar_modelo`). */
export interface ProgressoDoModelo {
  fase: "preparando" | "baixando" | "verificando" | "finalizando" | "concluido";
  baixadoEmBytes: number;
  totalEmBytes: number;
}

/** Falha do download (rejeição do comando `baixar_modelo`). */
export interface FalhaDoDownload {
  mensagem: string;
  podeContinuar: boolean;
}

/** Projeto, como o backend devolve. */
export interface Projeto {
  id: string;
  nome: string;
  systemPrompt: string;
  modeloPadrao: string | null;
  criadoEm: string;
  atualizadoEm: string;
}

/** Campos editáveis de um projeto. */
export interface DadosProjeto {
  nome: string;
  systemPrompt: string;
  modeloPadrao: string | null;
}

/** O que será apagado junto com um projeto. */
export interface ResumoExclusaoProjeto {
  conversas: number;
  mensagens: number;
  memorias: number;
}

/** Limites de tamanho, os mesmos que o backend aplica. */
export const NOME_DO_PROJETO_MAXIMO = 100;
export const INSTRUCOES_DO_PROJETO_MAXIMO = 20_000;

/** Texto de tamanho em MB ou GB, com vírgula decimal. */
export function formatarTamanho(bytes: number): string {
  const gigabyte = 1024 ** 3;
  const megabyte = 1024 ** 2;
  const valor = bytes >= gigabyte ? bytes / gigabyte : bytes / megabyte;
  const unidade = bytes >= gigabyte ? "GB" : "MB";
  return `${valor.toFixed(1).replace(".", ",")} ${unidade}`;
}

/** Conversa, como o backend devolve. Cada conversa guarda o próprio modelo e parâmetros. */
export interface Conversa {
  id: string;
  projetoId: string;
  titulo: string;
  modelo: string | null;
  temperature: number | null;
  numCtx: number | null;
  keepAlive: string | null;
  criadoEm: string;
  atualizadoEm: string;
}

/** Campos editáveis de uma conversa. */
export interface DadosConversa {
  titulo: string;
  modelo: string | null;
  temperature: number | null;
  numCtx: number | null;
  keepAlive: string | null;
}

export interface Mensagem {
  id: string;
  conversaId: string;
  posicao: number;
  papel: "user" | "assistant";
  conteudo: string;
  /** Raciocínio (`thinking`) do modelo, quando ele emitiu algum. */
  raciocinio: string | null;
  /** `interrompida`: a resposta foi guardada pela metade. */
  estado: "completa" | "interrompida";
  criadoEm: string;
  atualizadoEm: string;
}

/** Pedaço da resposta enviado durante a geração (canal dos comandos de chat). */
export type EventoDeGeracao =
  { tipo: "raciocinio"; texto: string } | { tipo: "conteudo"; texto: string };

/** Como a geração terminou (resultado dos comandos de chat). */
export interface ResultadoDaGeracao {
  estado: "concluida" | "cancelada";
  mensagem: Mensagem | null;
  /** O modelo parou por limite de tamanho. */
  cortada: boolean;
  /** O contexto enviado estava perto do `num_ctx` (ou além dele). */
  contexto?: AvisoDeContexto | null;
}

/** Tamanho estimado do contexto enviado e o `num_ctx` usado, em tokens. */
export interface AvisoDeContexto {
  estimado: number;
  limite: number;
}

/** Falha da geração (rejeição dos comandos de chat). */
export interface FalhaDaGeracao {
  mensagem: string;
  mensagemParcial: Mensagem | null;
}

export interface DetalhesDoModelo {
  janelaDeContexto: number | null;
  suportaRaciocinio: boolean;
}

/** Uma sugestão de memória do modelo, ainda sem escopo: o usuário escolhe ao aprovar. */
export interface SugestaoDeMemoria {
  texto: string;
}

export interface ResultadoDaSugestao {
  /** O usuário cancelou: não há sugestões e nada foi criado. */
  cancelada: boolean;
  sugestoes: SugestaoDeMemoria[];
}

export type EscopoDeMemoria = "global" | "projeto";

export interface Memoria {
  id: string;
  escopo: EscopoDeMemoria;
  /** Presente se, e somente se, o escopo é `projeto`. */
  projetoId: string | null;
  conteudo: string;
  criadoEm: string;
  atualizadoEm: string;
}

/** Tamanho máximo de uma memória, o mesmo que o backend aplica. */
export const MEMORIA_MAXIMA = 2_000;

/** Limite de tamanho de uma mensagem, o mesmo que o backend aplica. */
export const MENSAGEM_MAXIMA = 100_000;
export const TITULO_DA_CONVERSA_MAXIMO = 100;
