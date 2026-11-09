import { Channel, invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { formatarTamanho } from "./tipos";

/** Progresso enviado pelo backend durante a instalação. */
interface ProgressoInstalacao {
  fase:
    | "consultando"
    | "verificando_espaco"
    | "baixando"
    | "verificando"
    | "instalando"
    | "concluida";
  baixadoEmBytes: number;
  totalEmBytes: number;
}

const MENSAGEM_PADRAO =
  "Não foi possível instalar o Ollama. Feche o aplicativo, abra-o de novo e tente outra vez.";

const TEXTO_DA_FASE: Record<ProgressoInstalacao["fase"], string> = {
  consultando: "Procurando o instalador do Ollama…",
  verificando_espaco: "Verificando o espaço livre no disco…",
  baixando: "Baixando o instalador do Ollama…",
  verificando: "Conferindo se o instalador é autêntico…",
  instalando: "Instalando o Ollama. Isso pode levar alguns minutos…",
  concluida: "Instalação concluída.",
};

interface Props {
  /** Chamado quando a instalação termina com sucesso, para a tela voltar a detectar o Ollama. */
  aoConcluir: () => void;
}

/** Botão e progresso da instalação guiada do Ollama. */
function InstalarOllama({ aoConcluir }: Props) {
  const [instalando, setInstalando] = useState(false);
  const [progresso, setProgresso] = useState<ProgressoInstalacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [cancelada, setCancelada] = useState(false);

  async function instalar() {
    setInstalando(true);
    setErro(null);
    setCancelada(false);
    setProgresso(null);

    const canal = new Channel<ProgressoInstalacao>();
    canal.onmessage = setProgresso;
    try {
      const resultado = await invoke<string>("instalar_ollama", { canal });
      if (resultado === "concluida") {
        aoConcluir();
      } else {
        setCancelada(true);
      }
    } catch (falha: unknown) {
      setErro(typeof falha === "string" ? falha : MENSAGEM_PADRAO);
    } finally {
      setInstalando(false);
      setProgresso(null);
    }
  }

  function cancelar() {
    invoke("cancelar_instalacao_ollama").catch(() => undefined);
  }

  const baixando = progresso?.fase === "baixando" && progresso.totalEmBytes > 0;
  const percentual =
    baixando && progresso
      ? Math.min(
          100,
          Math.floor((progresso.baixadoEmBytes / progresso.totalEmBytes) * 100),
        )
      : null;

  return (
    <section aria-label="Instalação do Ollama">
      {erro && (
        <p role="alert" className="erro">
          {erro}
        </p>
      )}
      {cancelada && <p role="status">Instalação cancelada.</p>}

      {instalando ? (
        <>
          <p role="status">
            {progresso ? TEXTO_DA_FASE[progresso.fase] : "Preparando…"}
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
        <button type="button" onClick={() => void instalar()}>
          {erro || cancelada ? "Tentar de novo" : "Instalar o Ollama"}
        </button>
      )}
    </section>
  );
}

export default InstalarOllama;
