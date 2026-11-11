import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { mensagemDoErro } from "./erros";
import type { Conversa } from "./tipos";

interface Props {
  conversa: Conversa;
  /** Confirma a exclusão. Devolve a mensagem de erro, ou `null` se deu certo. */
  aoConfirmar: () => Promise<string | null>;
  aoCancelar: () => void;
}

/** Confirmação antes de apagar uma conversa, dizendo quantas mensagens somem com ela. */
function ConfirmarExclusaoDeConversa({
  conversa,
  aoConfirmar,
  aoCancelar,
}: Props) {
  const [mensagens, setMensagens] = useState<number | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [excluindo, setExcluindo] = useState(false);

  useEffect(() => {
    let ativo = true;
    invoke<number>("resumo_exclusao_conversa", { id: conversa.id })
      .then((quantidade) => {
        if (ativo) {
          setMensagens(quantidade);
        }
      })
      .catch((falha: unknown) => {
        if (ativo) {
          setErro(
            mensagemDoErro(
              falha,
              "Não foi possível verificar o que será apagado.",
            ),
          );
        }
      });
    return () => {
      ativo = false;
    };
  }, [conversa.id]);

  async function confirmar() {
    setExcluindo(true);
    setErro(null);
    const falha = await aoConfirmar();
    setExcluindo(false);
    if (falha) {
      setErro(falha);
    }
  }

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
            Excluir a conversa &quot;{conversa.titulo}&quot;?
          </AlertDialog.Title>
          <AlertDialog.Description asChild>
            <div className="mt-2 space-y-2 text-sm">
              {mensagens === null && !erro && (
                <p>Verificando o que será apagado…</p>
              )}
              {mensagens !== null && mensagens > 0 && (
                <p>
                  Isto apaga {mensagens}{" "}
                  {mensagens === 1 ? "mensagem" : "mensagens"} desta conversa.
                </p>
              )}
              {mensagens === 0 && <p>Esta conversa ainda não tem mensagens.</p>}
              <p>Esta ação não pode ser desfeita.</p>
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
              disabled={mensagens === null || excluindo}
              onClick={() => void confirmar()}
              className="rounded bg-red-700 px-3 py-1 text-white disabled:opacity-50"
            >
              Excluir conversa
            </button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

export default ConfirmarExclusaoDeConversa;
