import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { descreverItens, totalDe } from "./itensDeBackup";
import type { AnaliseDoBackup } from "./tipos";

interface Props {
  analise: AnaliseDoBackup;
  importando: boolean;
  /** Mensagem amigável se a importação falhou. */
  erro: string | null;
  aoConfirmar: () => void;
  aoCancelar: () => void;
}

function dataDoBackup(texto: string): string {
  const data = new Date(texto);
  return Number.isNaN(data.getTime()) ? texto : data.toLocaleString("pt-BR");
}

/**
 * Confirmação da importação. Mostra o que o backup traz, antes de gravar qualquer coisa:
 * o que será adicionado, o que já existe aqui (e fica como está) e o que foi excluído aqui
 * (e não volta). Importar nunca apaga nem altera o que já existe.
 */
function ConfirmarImportacao({
  analise,
  importando,
  erro,
  aoConfirmar,
  aoCancelar,
}: Props) {
  const { resumo } = analise;
  const novos = {
    projetos: resumo.projetos.novos,
    conversas: resumo.conversas.novos,
    mensagens: resumo.mensagens.novos,
    memorias: resumo.memorias.novos,
  };
  const jaExistem = {
    projetos: resumo.projetos.jaExistem,
    conversas: resumo.conversas.jaExistem,
    mensagens: resumo.mensagens.jaExistem,
    memorias: resumo.memorias.jaExistem,
  };
  const ignorados = {
    projetos: resumo.projetos.ignorados,
    conversas: resumo.conversas.ignorados,
    mensagens: resumo.mensagens.ignorados,
    memorias: resumo.memorias.ignorados,
  };
  const temNovidade = totalDe(novos) > 0;

  return (
    <AlertDialog.Root
      open
      onOpenChange={(aberto) => {
        if (!aberto && !importando) {
          aoCancelar();
        }
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 bg-black/50" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 w-[min(32rem,90vw)] -translate-x-1/2 -translate-y-1/2 rounded-lg bg-white p-5 shadow-xl dark:bg-neutral-900">
          <AlertDialog.Title className="text-lg font-semibold">
            Importar este backup?
          </AlertDialog.Title>
          <AlertDialog.Description asChild>
            <div className="mt-2 space-y-2 text-sm">
              <p>Backup feito em {dataDoBackup(analise.exportadoEm)}.</p>
              {temNovidade ? (
                <p>
                  <strong>Vai adicionar:</strong> {descreverItens(novos)}.
                </p>
              ) : (
                <p>
                  <strong>Não há nada novo para importar.</strong> Tudo o que
                  está neste backup já existe aqui, ou foi excluído.
                </p>
              )}
              {totalDe(jaExistem) > 0 && (
                <p>
                  <strong>Já existem aqui e ficam como estão:</strong>{" "}
                  {descreverItens(jaExistem)}.
                </p>
              )}
              {totalDe(ignorados) > 0 && (
                <p>
                  <strong>Foram excluídos aqui e não voltam:</strong>{" "}
                  {descreverItens(ignorados)}.
                </p>
              )}
              <p>Nada do que você já tem é apagado nem alterado.</p>
            </div>
          </AlertDialog.Description>

          {erro && (
            <p role="alert" className="erro mt-2 text-sm">
              {erro}
            </p>
          )}

          <div className="mt-4 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                disabled={importando}
                className="rounded border border-neutral-400 px-3 py-1 disabled:opacity-50"
              >
                {temNovidade ? "Cancelar" : "Fechar"}
              </button>
            </AlertDialog.Cancel>
            {temNovidade && (
              <button
                type="button"
                disabled={importando}
                onClick={aoConfirmar}
                className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-50"
              >
                {importando ? "Importando…" : "Importar"}
              </button>
            )}
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

export default ConfirmarImportacao;
