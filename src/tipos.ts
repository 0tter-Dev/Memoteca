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
