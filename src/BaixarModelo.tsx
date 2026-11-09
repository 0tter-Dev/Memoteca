import { Channel, invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
  formatarTamanho,
  type FalhaDoDownload,
  type ProgressoDoModelo,
} from "./tipos";

const MENSAGEM_PADRAO =
  "Não foi possível baixar o modelo. Feche o aplicativo, abra-o de novo e tente outra vez.";

const TEXTO_DA_FASE: Record<ProgressoDoModelo["fase"], string> = {
  preparando: "Preparando o download…",
  baixando: "Baixando o modelo…",
  verificando: "Conferindo o arquivo baixado…",
  finalizando: "Finalizando…",
  concluido: "Download concluído.",
};

interface Props {
  nome: string;
  /** O modelo foi baixado e conferido. */
  aoConcluir: () => void;
  /** O usuário quer escolher outro modelo. */
  aoEscolherOutro: () => void;
}

/** Passo "Download" do onboarding: progresso, cancelar e recuperação. */
function BaixarModelo({ nome, aoConcluir, aoEscolherOutro }: Props) {
  const [baixando, setBaixando] = useState(false);
  const [progresso, setProgresso] = useState<ProgressoDoModelo | null>(null);
  const [falha, setFalha] = useState<FalhaDoDownload | null>(null);
  const [cancelado, setCancelado] = useState(false);
  const jaIniciou = useRef(false);

  async function baixar() {
    setBaixando(true);
    setFalha(null);
    setCancelado(false);
    setProgresso(null);

    const canal = new Channel<ProgressoDoModelo>();
    canal.onmessage = setProgresso;
    try {
      const resultado = await invoke<string>("baixar_modelo", { nome, canal });
      if (resultado === "concluido") {
        aoConcluir();
      } else {
        setCancelado(true);
      }
    } catch (erro: unknown) {
      const tipada = erro as Partial<FalhaDoDownload> | null;
      setFalha({
        mensagem:
          typeof tipada?.mensagem === "string"
            ? tipada.mensagem
            : MENSAGEM_PADRAO,
        podeContinuar: tipada?.podeContinuar === true,
      });
    } finally {
      setBaixando(false);
      setProgresso(null);
    }
  }

  // Começa o download ao entrar no passo. A guarda evita iniciar duas vezes quando o React
  // reexecuta o efeito em desenvolvimento.
  useEffect(() => {
    if (!jaIniciou.current) {
      jaIniciou.current = true;
      void baixar();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function cancelar() {
    invoke("cancelar_download_modelo").catch(() => undefined);
  }

  const total = progresso?.totalEmBytes ?? 0;
  const percentual =
    progresso?.fase === "baixando" && total > 0
      ? Math.min(100, Math.floor((progresso.baixadoEmBytes / total) * 100))
      : null;
  const rotuloDeRetomar =
    falha?.podeContinuar || cancelado
      ? "Continuar o download"
      : "Tentar de novo";

  return (
    <section aria-label="Download do modelo">
      <h2>Baixando {nome}</h2>
      {falha && (
        <p role="alert" className="erro">
          {falha.mensagem}
        </p>
      )}
      {cancelado && (
        <p role="status">
          Download cancelado. O que já foi baixado fica guardado, e você pode
          continuar depois.
        </p>
      )}

      {baixando ? (
        <>
          <p role="status">
            {progresso
              ? TEXTO_DA_FASE[progresso.fase]
              : "Preparando o download…"}
          </p>
          {percentual !== null && progresso && (
            <p>
              <progress
                aria-label="Progresso do download"
                max={100}
                value={percentual}
              />{" "}
              {percentual}% ({formatarTamanho(progresso.baixadoEmBytes)} de{" "}
              {formatarTamanho(progresso.totalEmBytes)})
            </p>
          )}
          <button type="button" onClick={cancelar}>
            Cancelar
          </button>
        </>
      ) : (
        <p>
          <button type="button" onClick={() => void baixar()}>
            {rotuloDeRetomar}
          </button>{" "}
          <button type="button" onClick={aoEscolherOutro}>
            Escolher outro modelo
          </button>
        </p>
      )}
    </section>
  );
}

export default BaixarModelo;
