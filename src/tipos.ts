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

/** Texto de tamanho em MB ou GB, com vírgula decimal. */
export function formatarTamanho(bytes: number): string {
  const gigabyte = 1024 ** 3;
  const megabyte = 1024 ** 2;
  const valor = bytes >= gigabyte ? bytes / gigabyte : bytes / megabyte;
  const unidade = bytes >= gigabyte ? "GB" : "MB";
  return `${valor.toFixed(1).replace(".", ",")} ${unidade}`;
}
