import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { Projeto, ResumoExclusaoProjeto } from "./tipos";

interface Props {
  projeto: Projeto;
  /** Confirma a exclusão. Devolve a mensagem de erro, ou `null` se deu certo. */
  aoConfirmar: () => Promise<string | null>;
  aoCancelar: () => void;
}

function plural(quantidade: number, singular: string, plural: string) {
  return `${quantidade} ${quantidade === 1 ? singular : plural}`;
}

/**
 * Confirmação antes de apagar um projeto. Mostra o que será apagado junto: as conversas, as
 * mensagens e as memórias do projeto. As memórias globais não são afetadas.
 */
function ConfirmarExclusao({ projeto, aoConfirmar, aoCancelar }: Props) {
  const [resumo, setResumo] = useState<ResumoExclusaoProjeto | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [excluindo, setExcluindo] = useState(false);

  useEffect(() => {
    let ativo = true;
    invoke<ResumoExclusaoProjeto>("resumo_exclusao_projeto", { id: projeto.id })
      .then((dados) => {
        if (ativo) {
          setResumo(dados);
        }
      })
      .catch((falha: unknown) => {
        if (ativo) {
          setErro(
            typeof falha === "string"
              ? falha
              : "Não foi possível verificar o que será apagado.",
          );
        }
      });
    return () => {
      ativo = false;
    };
  }, [projeto.id]);

  async function confirmar() {
    setExcluindo(true);
    setErro(null);
    const falha = await aoConfirmar();
    setExcluindo(false);
    if (falha) {
      setErro(falha);
    }
  }

  const nadaAlemDoProjeto =
    resumo !== null &&
    resumo.conversas === 0 &&
    resumo.mensagens === 0 &&
    resumo.memorias === 0;

  return (
    <AlertDialog.Root
      open
      onOpenChange={(aberto) => {
        if (!aberto) {
          aoCancelar();
        }
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 bg-black/50" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 w-[min(28rem,90vw)] -translate-x-1/2 -translate-y-1/2 rounded-lg bg-white p-5 shadow-xl dark:bg-neutral-900">
          <AlertDialog.Title className="text-lg font-semibold">
            Excluir o projeto &quot;{projeto.nome}&quot;?
          </AlertDialog.Title>
          <AlertDialog.Description asChild>
            <div className="mt-2 space-y-2 text-sm">
              {resumo === null && !erro && (
                <p>Verificando o que será apagado…</p>
              )}
              {resumo && !nadaAlemDoProjeto && (
                <p>
                  Isto apaga também{" "}
                  {plural(resumo.conversas, "conversa", "conversas")},{" "}
                  {plural(resumo.mensagens, "mensagem", "mensagens")} e{" "}
                  {plural(resumo.memorias, "memória", "memórias")} deste
                  projeto.
                </p>
              )}
              {nadaAlemDoProjeto && (
                <p>Este projeto ainda não tem conversas nem memórias.</p>
              )}
              <p>
                As memórias globais não são afetadas. Esta ação não pode ser
                desfeita.
              </p>
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
                className="rounded border border-neutral-400 px-3 py-1"
              >
                Cancelar
              </button>
            </AlertDialog.Cancel>
            <button
              type="button"
              disabled={resumo === null || excluindo}
              onClick={() => void confirmar()}
              className="rounded bg-red-700 px-3 py-1 text-white disabled:opacity-50"
            >
              Excluir projeto
            </button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

export default ConfirmarExclusao;
