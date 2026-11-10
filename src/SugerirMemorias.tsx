import { useEffect, useRef } from "react";
import { useSugestoes, type Sugestao } from "./estado/sugestoes";
import type { Conversa, EscopoDeMemoria } from "./tipos";

interface ItemProps {
  sugestao: Sugestao;
  projetoId: string;
}

function ItemDeSugestao({ sugestao, projetoId }: ItemProps) {
  const mudarEscopo = useSugestoes((estado) => estado.mudarEscopo);
  const aprovar = useSugestoes((estado) => estado.aprovar);
  const rejeitar = useSugestoes((estado) => estado.rejeitar);
  const nome = `escopo-${sugestao.id}`;

  return (
    <li className="space-y-2 rounded border border-neutral-300 p-2 dark:border-neutral-700">
      <p className="whitespace-pre-wrap">{sugestao.texto}</p>
      <fieldset className="flex flex-wrap gap-3 text-sm">
        <legend className="sr-only">Onde guardar: {sugestao.texto}</legend>
        {(
          [
            ["projeto", "Deste projeto"],
            ["global", "Global"],
          ] as [EscopoDeMemoria, string][]
        ).map(([valor, rotulo]) => (
          <label key={valor} className="flex items-center gap-1">
            <input
              type="radio"
              name={nome}
              value={valor}
              checked={sugestao.escopo === valor}
              disabled={sugestao.salvando}
              onChange={() => mudarEscopo(sugestao.id, valor)}
            />
            {rotulo}
          </label>
        ))}
      </fieldset>
      <div className="flex gap-2">
        <button
          type="button"
          disabled={sugestao.salvando}
          onClick={() => void aprovar(sugestao.id, projetoId)}
          aria-label={`Aprovar a sugestão “${sugestao.texto}”`}
          className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-60"
        >
          Aprovar
        </button>
        <button
          type="button"
          disabled={sugestao.salvando}
          onClick={() => rejeitar(sugestao.id)}
          aria-label={`Rejeitar a sugestão “${sugestao.texto}”`}
          className="rounded border border-neutral-400 px-3 py-1 disabled:opacity-60"
        >
          Rejeitar
        </button>
      </div>
      {sugestao.erro && (
        <p role="alert" className="erro text-sm">
          {sugestao.erro}
        </p>
      )}
    </li>
  );
}

interface Props {
  conversa: Conversa;
}

/**
 * Diálogo de sugestões de memória. O modelo propõe, e **só a aprovação de cada sugestão cria
 * uma memória**. Fechar o diálogo (ou rejeitar) não cria nada.
 */
function SugerirMemorias({ conversa }: Props) {
  const {
    aberta,
    gerando,
    concluiu,
    erro,
    sugestoes,
    aprovadas,
    fechar,
    tentarDeNovo,
  } = useSugestoes();
  const painel = useRef<HTMLDivElement>(null);

  // Leva o foco para o diálogo ao abrir, e Esc fecha.
  useEffect(() => {
    if (!aberta) {
      return;
    }
    painel.current?.focus();
    function aoTeclar(evento: KeyboardEvent) {
      if (evento.key === "Escape") {
        fechar();
      }
    }
    document.addEventListener("keydown", aoTeclar);
    return () => document.removeEventListener("keydown", aoTeclar);
  }, [aberta, fechar]);

  if (!aberta) {
    return null;
  }

  const acabou = concluiu && sugestoes.length === 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div
        ref={painel}
        role="dialog"
        aria-modal="true"
        aria-labelledby="titulo-das-sugestoes"
        tabIndex={-1}
        className="max-h-full w-full max-w-xl space-y-3 overflow-y-auto rounded bg-white p-4 text-neutral-900 shadow-lg dark:bg-neutral-900 dark:text-neutral-100"
      >
        <h2 id="titulo-das-sugestoes" className="text-lg font-semibold">
          Sugestões de memória
        </h2>

        {gerando && (
          <div className="space-y-2">
            <p role="status">Analisando a conversa com o modelo…</p>
            <button
              type="button"
              onClick={fechar}
              className="rounded border border-neutral-400 px-3 py-1"
            >
              Cancelar
            </button>
          </div>
        )}

        {!gerando && erro && (
          <div className="space-y-2">
            <p role="alert" className="erro">
              {erro}
            </p>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => void tentarDeNovo()}
                className="rounded bg-blue-700 px-3 py-1 text-white"
              >
                Tentar de novo
              </button>
              <button
                type="button"
                onClick={fechar}
                className="rounded border border-neutral-400 px-3 py-1"
              >
                Fechar
              </button>
            </div>
          </div>
        )}

        {!gerando && !erro && concluiu && sugestoes.length > 0 && (
          <>
            <p className="text-neutral-600 dark:text-neutral-400">
              Nada vira memória sem a sua aprovação. Escolha onde guardar cada
              uma e aprove, ou rejeite. Fechar sem decidir não guarda nada.
            </p>
            <ul aria-label="Sugestões" className="space-y-2">
              {sugestoes.map((sugestao) => (
                <ItemDeSugestao
                  key={sugestao.id}
                  sugestao={sugestao}
                  projetoId={conversa.projetoId}
                />
              ))}
            </ul>
          </>
        )}

        {!gerando && !erro && acabou && (
          <p>
            {aprovadas > 0
              ? `Pronto: ${aprovadas} ${aprovadas === 1 ? "memória guardada" : "memórias guardadas"}.`
              : "O modelo não encontrou nada que valha a pena guardar desta conversa."}
          </p>
        )}

        {!gerando && !erro && (
          <button
            type="button"
            onClick={fechar}
            className="rounded border border-neutral-400 px-3 py-1"
          >
            Fechar
          </button>
        )}
      </div>
    </div>
  );
}

export default SugerirMemorias;
