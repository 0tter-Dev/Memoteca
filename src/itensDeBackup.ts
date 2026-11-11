/** Quantos itens de cada tipo, para falar de um backup em português. */
export interface Itens {
  projetos: number;
  conversas: number;
  mensagens: number;
  memorias: number;
}

export function totalDe(itens: Itens): number {
  return itens.projetos + itens.conversas + itens.mensagens + itens.memorias;
}

function parte(quantidade: number, singular: string, plural: string) {
  return quantidade === 1 ? `1 ${singular}` : `${quantidade} ${plural}`;
}

/** "2 projetos, 1 conversa, 10 mensagens e 3 memórias", sem os tipos que estão em zero. */
export function descreverItens(itens: Itens): string {
  const partes = [
    itens.projetos > 0 ? parte(itens.projetos, "projeto", "projetos") : null,
    itens.conversas > 0
      ? parte(itens.conversas, "conversa", "conversas")
      : null,
    itens.mensagens > 0
      ? parte(itens.mensagens, "mensagem", "mensagens")
      : null,
    itens.memorias > 0 ? parte(itens.memorias, "memória", "memórias") : null,
  ].filter((texto): texto is string => texto !== null);

  if (partes.length === 0) {
    return "nenhum item";
  }
  if (partes.length === 1) {
    return partes[0];
  }
  return `${partes.slice(0, -1).join(", ")} e ${partes[partes.length - 1]}`;
}
