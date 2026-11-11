import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { mensagemDoErro } from "./erros";
import Onboarding from "./Onboarding";
import TelaPrincipal from "./TelaPrincipal";
import type { EstadoOllama } from "./tipos";

const MENSAGEM_DADOS =
  "Não foi possível verificar os dados do Memoteca. Tente de novo.";
const MENSAGEM_OLLAMA = "Não foi possível verificar o Ollama. Tente de novo.";
const MENSAGEM_PASTA =
  "Não foi possível abrir a pasta dos dados. Abra o Explorador de Arquivos e procure a pasta do Memoteca dentro de AppData.";

interface FalhaDosDadosProps {
  mensagem: string;
  aoTentarDeNovo: () => Promise<void>;
}

/**
 * Os dados do Memoteca não abriram (arquivo travado, protegido ou danificado). Nada foi
 * apagado. Dá para tentar de novo **sem reiniciar o app** e para abrir a pasta onde ficam o
 * banco, as cópias de segurança e o log.
 */
function FalhaDosDados({ mensagem, aoTentarDeNovo }: FalhaDosDadosProps) {
  const [tentando, setTentando] = useState(false);
  const [erroDaPasta, setErroDaPasta] = useState<string | null>(null);

  async function tentar() {
    setTentando(true);
    setErroDaPasta(null);
    await aoTentarDeNovo();
    setTentando(false);
  }

  async function abrirPasta() {
    setErroDaPasta(null);
    try {
      await invoke("abrir_pasta_de_dados");
    } catch (falha: unknown) {
      setErroDaPasta(mensagemDoErro(falha, MENSAGEM_PASTA));
    }
  }

  return (
    <main className="app">
      <h1>Memoteca</h1>
      <section
        role="alertdialog"
        aria-labelledby="titulo-falha-dos-dados"
        aria-describedby="texto-falha-dos-dados"
        className="max-w-xl space-y-3"
      >
        <h2 id="titulo-falha-dos-dados" className="text-xl font-semibold">
          Não foi possível abrir seus dados
        </h2>
        <p id="texto-falha-dos-dados" role="alert" className="erro">
          {mensagem}
        </p>
        <p className="text-neutral-600 dark:text-neutral-400">
          Seus dados continuam no computador. Se o problema persistir, a pasta
          dos dados tem o banco, as cópias de segurança e o log.
        </p>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={tentando}
            onClick={() => void tentar()}
            className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-60"
          >
            {tentando ? "Tentando…" : "Tentar de novo"}
          </button>
          <button
            type="button"
            onClick={() => void abrirPasta()}
            className="rounded border border-neutral-400 px-3 py-1"
          >
            Abrir a pasta dos dados
          </button>
        </div>
        {erroDaPasta && (
          <p role="alert" className="erro text-sm">
            {erroDaPasta}
          </p>
        )}
      </section>
    </main>
  );
}

function App() {
  const [erroDeDados, setErroDeDados] = useState<string | null>(null);
  const [ollama, setOllama] = useState<EstadoOllama | null>(null);
  const [erroDoOllama, setErroDoOllama] = useState<string | null>(null);
  /** `null` enquanto ainda não se sabe. */
  const [onboardingConcluido, setOnboardingConcluido] = useState<
    boolean | null
  >(null);
  const [jaConcluiuAntes, setJaConcluiuAntes] = useState(false);
  /** Sobe a cada vez que os dados voltam a abrir, para a tela ler tudo de novo. */
  const [rodadaDosDados, setRodadaDosDados] = useState(0);

  // O backend abre o banco ao iniciar. Se falhar, ele devolve a mensagem amigável.
  useEffect(() => {
    let ativo = true;
    invoke("estado_do_banco").catch((erro: unknown) => {
      if (ativo) {
        setErroDeDados(mensagemDoErro(erro, MENSAGEM_DADOS));
      }
    });
    return () => {
      ativo = false;
    };
  }, []);

  // "Tentar de novo": abre o banco sem reiniciar o app e, se der certo, lê tudo de novo.
  const tentarAbrirOsDados = useCallback(async () => {
    try {
      await invoke("tentar_abrir_banco");
      setErroDeDados(null);
      setOnboardingConcluido(null);
      setRodadaDosDados((rodada) => rodada + 1);
    } catch (erro: unknown) {
      setErroDeDados(mensagemDoErro(erro, MENSAGEM_DADOS));
    }
  }, []);

  // Já passou pelo onboarding antes?
  useEffect(() => {
    let ativo = true;
    invoke<boolean>("onboarding_concluido")
      .then((concluido) => {
        if (ativo) {
          setOnboardingConcluido(concluido);
          setJaConcluiuAntes(concluido);
        }
      })
      // Sem poder ler a configuração, mostra o onboarding, que é seguro.
      .catch(() => {
        if (ativo) {
          setOnboardingConcluido(false);
        }
      });
    return () => {
      ativo = false;
    };
  }, [rodadaDosDados]);

  // Detecta o Ollama, reutiliza o que já roda ou inicia o instalado.
  const prepararOllama = useCallback(() => {
    invoke<EstadoOllama>("preparar_ollama")
      .then((estado) => {
        setErroDoOllama(null);
        setOllama(estado);
      })
      .catch((erro: unknown) => {
        setErroDoOllama(mensagemDoErro(erro, MENSAGEM_OLLAMA));
      });
  }, []);

  useEffect(() => {
    prepararOllama();
  }, [prepararOllama]);

  // Os dados não abriram: diálogo com tentar de novo e a pasta dos dados.
  if (erroDeDados) {
    return (
      <FalhaDosDados
        mensagem={erroDeDados}
        aoTentarDeNovo={tentarAbrirOsDados}
      />
    );
  }

  // Erro ao verificar o Ollama: não dá para seguir, mas dá para tentar de novo.
  if (erroDoOllama) {
    return (
      <main className="app">
        <h1>Memoteca</h1>
        <p role="alert" className="erro">
          {erroDoOllama}
        </p>
        <button
          type="button"
          onClick={() => {
            setErroDoOllama(null);
            prepararOllama();
          }}
          className="mt-2 rounded bg-blue-700 px-3 py-1 text-white"
        >
          Tentar de novo
        </button>
      </main>
    );
  }

  if (ollama === null || onboardingConcluido === null) {
    return (
      <main className="app">
        <h1>Memoteca</h1>
        <p role="status">Carregando…</p>
      </main>
    );
  }

  const precisaDeOnboarding =
    !onboardingConcluido ||
    ollama.estado !== "em_execucao" ||
    (ollama.modelosInstalados ?? 0) === 0;

  if (precisaDeOnboarding) {
    return (
      <main className="app">
        <h1>Memoteca</h1>
        <Onboarding
          ollama={ollama}
          jaConcluiuAntes={jaConcluiuAntes}
          aoAtualizarOllama={prepararOllama}
          aoConcluir={() => setOnboardingConcluido(true)}
        />
      </main>
    );
  }

  return (
    <TelaPrincipal key={rodadaDosDados} ollama={ollama} erroDeDados={null} />
  );
}

export default App;
