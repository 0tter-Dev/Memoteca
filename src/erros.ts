/**
 * Mensagens de erro que chegam do backend à tela.
 *
 * O backend só devolve mensagens amigáveis em pt-BR (os detalhes técnicos vão para o log
 * local). Aqui fica a **segunda barreira**: se algo que não é uma mensagem amigável chegar
 * (um erro do próprio Tauri, de argumento inválido ou de comando inexistente, uma exceção do
 * JavaScript, um texto com cara de erro técnico), a tela mostra a mensagem padrão em vez do
 * texto cru.
 */

export const MENSAGEM_PADRAO =
  "Não foi possível concluir. Feche o aplicativo, abra-o de novo e tente outra vez.";

/** Padrões que denunciam texto técnico: o que o usuário nunca deve ver. */
const PADROES_TECNICOS: RegExp[] = [
  // Erros do Tauri e do serde.
  /invalid args/i,
  /missing field/i,
  /unknown (variant|field)/i,
  /command \S+ not found/i,
  /not allowed|not a valid|invalid type/i,
  // Exceções e pilhas do JavaScript.
  /\b(Type|Reference|Syntax|Range|Eval|URI)Error\b/,
  /\bError:/,
  /\bException\b/i,
  /\bat \S+ \(.*:\d+:\d+\)/,
  /\bat .*:\d+:\d+\b/,
  /\bundefined\b|\bNaN\b/,
  // Rust, SQLite e HTTP.
  /panicked|backtrace|stack trace/i,
  /\bSQLITE\w*|\bsqlite\b/i,
  /\bHTTP\b|\b[45]\d\d\b (Not Found|Bad|Internal|Service)/i,
  /::/,
  // JSON e objetos.
  /^\s*[{[]/,
  /[}\]]\s*$/,
  /\[object /,
  // Caminhos de arquivo e endereços locais.
  /\b[A-Za-z]:\\/,
  /\\\\\?\\/,
  /127\.0\.0\.1|localhost/i,
  // Inglês solto.
  /\b(failed|cannot|unable|denied|refused|timed out|timeout)\b/i,
];

/** O texto parece um erro técnico, e não uma mensagem para o usuário? */
export function pareceTecnica(texto: string): boolean {
  return PADROES_TECNICOS.some((padrao) => padrao.test(texto));
}

/**
 * A mensagem para mostrar ao usuário. Só texto amigável passa: qualquer outra coisa (exceção,
 * objeto, texto vazio ou com cara de erro técnico) vira `padrao`.
 */
export function mensagemDoErro(
  erro: unknown,
  padrao: string = MENSAGEM_PADRAO,
): string {
  if (typeof erro !== "string") {
    return padrao;
  }
  const texto = erro.trim();
  return texto === "" || pareceTecnica(texto) ? padrao : texto;
}

/**
 * Como `mensagemDoErro`, para falhas que chegam como objeto com `mensagem` (a geração de
 * resposta e o download de modelo devolvem `{ mensagem, ... }`).
 */
export function mensagemDaFalha(
  erro: unknown,
  padrao: string = MENSAGEM_PADRAO,
): string {
  if (typeof erro === "object" && erro !== null && "mensagem" in erro) {
    return mensagemDoErro((erro as { mensagem: unknown }).mensagem, padrao);
  }
  return mensagemDoErro(erro, padrao);
}
