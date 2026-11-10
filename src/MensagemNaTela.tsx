import Markdown from "./Markdown";
import type { Mensagem } from "./tipos";

/** O raciocínio (`thinking`) fica recolhido por padrão e só aparece se o modelo o emitiu. */
function Raciocinio({ texto, titulo }: { texto: string; titulo: string }) {
  if (texto.trim() === "") {
    return null;
  }
  return (
    <details className="mb-2 text-sm text-neutral-600 dark:text-neutral-400">
      <summary className="cursor-pointer">{titulo}</summary>
      <p className="mt-1 whitespace-pre-wrap">{texto}</p>
    </details>
  );
}

interface Props {
  mensagem: Mensagem;
  /** Mostra o botão de gerar de novo (só na última resposta, fora de uma geração). */
  podeRegenerar: boolean;
  aoRegenerar: () => void;
}

/** Uma mensagem da conversa: a do usuário em texto simples e a do assistente em markdown. */
function MensagemNaTela({ mensagem, podeRegenerar, aoRegenerar }: Props) {
  if (mensagem.papel === "user") {
    return (
      <article
        aria-label="Você"
        className="ml-auto max-w-[85%] rounded-lg bg-blue-100 px-3 py-2 dark:bg-blue-950"
      >
        <p className="whitespace-pre-wrap">{mensagem.conteudo}</p>
      </article>
    );
  }

  return (
    <article aria-label="Assistente" className="max-w-[90%]">
      <Raciocinio texto={mensagem.raciocinio ?? ""} titulo="Raciocínio" />
      <Markdown texto={mensagem.conteudo} />
      {mensagem.estado === "interrompida" && (
        <p role="status" className="aviso mt-1 text-sm">
          Resposta interrompida.
        </p>
      )}
      {podeRegenerar && (
        <button
          type="button"
          onClick={aoRegenerar}
          className="mt-1 rounded border border-neutral-400 px-2 py-0.5 text-sm"
        >
          Gerar de novo
        </button>
      )}
    </article>
  );
}

/** A resposta que está chegando agora, antes de ser gravada. */
export function RespostaChegando({
  conteudo,
  raciocinio,
}: {
  conteudo: string;
  raciocinio: string;
}) {
  return (
    <article aria-label="Assistente" className="max-w-[90%]">
      <Raciocinio texto={raciocinio} titulo="Raciocínio (pensando…)" />
      {conteudo !== "" && <Markdown texto={conteudo} />}
      <p
        role="status"
        className="text-sm text-neutral-600 dark:text-neutral-400"
      >
        Gerando a resposta…
      </p>
    </article>
  );
}

export default MensagemNaTela;
