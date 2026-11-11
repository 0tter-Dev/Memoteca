import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import BaixarModelo from "./BaixarModelo";
import EscolherModelo from "./EscolherModelo";
import InstalarOllama from "./InstalarOllama";
import type { EstadoOllama } from "./tipos";

type Passo = "boas_vindas" | "ollama" | "modelo" | "download" | "pronto";

interface Props {
  /** Estado do Ollama, ou `null` enquanto ainda está sendo verificado. */
  ollama: EstadoOllama | null;
  /** Quem já concluiu o onboarding antes pula as boas-vindas. */
  jaConcluiuAntes: boolean;
  /** Pede ao backend para verificar o Ollama de novo (depois de instalar ou baixar). */
  aoAtualizarOllama: () => void;
  /** O usuário clicou em "Começar": o onboarding acabou. */
  aoConcluir: () => void;
}

import { mensagemDoErro } from "./erros";

const MENSAGEM_PADRAO =
  "Não foi possível concluir. Feche o aplicativo, abra-o de novo e tente outra vez.";

/** Onboarding guiado: boas-vindas, Ollama, modelo, download e pronto. */
function Onboarding({
  ollama,
  jaConcluiuAntes,
  aoAtualizarOllama,
  aoConcluir,
}: Props) {
  const [passo, setPasso] = useState<Passo>(
    jaConcluiuAntes ? "ollama" : "boas_vindas",
  );
  const [modelo, setModelo] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  function seguirDoOllama() {
    setPasso((ollama?.modelosInstalados ?? 0) > 0 ? "pronto" : "modelo");
  }

  async function comecar() {
    setErro(null);
    try {
      await invoke("concluir_onboarding");
      aoConcluir();
    } catch (falha: unknown) {
      setErro(mensagemDoErro(falha, MENSAGEM_PADRAO));
    }
  }

  if (passo === "boas_vindas") {
    return (
      <section aria-label="Boas-vindas">
        <h2>Bem-vindo ao Memoteca</h2>
        <p>
          O Memoteca organiza suas conversas com modelos de linguagem que rodam
          no seu próprio computador, em projetos, e guarda memórias que você
          controla. Vamos deixar tudo pronto em poucos passos, sem precisar
          abrir um terminal.
        </p>
        <p role="note">
          O Ollama é um software separado, obtido do site oficial. O Memoteca é
          independente e não é afiliado a ele.
        </p>
        <button type="button" onClick={() => setPasso("ollama")}>
          Continuar
        </button>
      </section>
    );
  }

  if (passo === "ollama") {
    return (
      <section aria-label="Ollama">
        <h2>O Ollama</h2>
        {ollama === null && <p role="status">Verificando o Ollama…</p>}

        {ollama?.estado === "nao_instalado" && (
          <>
            <p>
              O Memoteca usa o Ollama para rodar os modelos. Ele ainda não está
              instalado neste computador. Podemos instalá-lo para você: o
              arquivo é baixado do site oficial e conferido antes de ser
              executado.
            </p>
            <InstalarOllama aoConcluir={aoAtualizarOllama} />
          </>
        )}

        {(ollama?.estado === "porta_ocupada" ||
          ollama?.estado === "indisponivel") && (
          <>
            <p role="alert" className="erro">
              {ollama.mensagem}
            </p>
            <button type="button" onClick={aoAtualizarOllama}>
              Verificar de novo
            </button>
          </>
        )}

        {ollama?.estado === "em_execucao" && (
          <>
            <p role="status">Ollama {ollama.versao} em execução.</p>
            {ollama.aviso && (
              <p role="status" className="aviso">
                {ollama.aviso}
              </p>
            )}
            <button type="button" onClick={seguirDoOllama}>
              Continuar
            </button>
          </>
        )}
      </section>
    );
  }

  if (passo === "modelo") {
    return (
      <EscolherModelo
        aoBaixar={(nome) => {
          setModelo(nome);
          setPasso("download");
        }}
      />
    );
  }

  if (passo === "download" && modelo) {
    return (
      <BaixarModelo
        nome={modelo}
        aoConcluir={() => {
          aoAtualizarOllama();
          setPasso("pronto");
        }}
        aoEscolherOutro={() => setPasso("modelo")}
      />
    );
  }

  return (
    <section aria-label="Pronto">
      <h2>Tudo pronto</h2>
      <p>
        O Ollama está funcionando e você já tem um modelo para conversar. Vamos
        começar.
      </p>
      {erro && (
        <p role="alert" className="erro">
          {erro}
        </p>
      )}
      <button type="button" onClick={() => void comecar()}>
        Começar
      </button>
    </section>
  );
}

export default Onboarding;
