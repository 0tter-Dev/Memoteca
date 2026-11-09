import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import Onboarding from "./Onboarding";
import type { EstadoOllama } from "./tipos";

const MENSAGEM_DADOS =
  "Não foi possível verificar os dados do Memoteca. Feche o aplicativo e abra-o de novo.";
const MENSAGEM_OLLAMA =
  "Não foi possível verificar o Ollama. Feche o aplicativo e abra-o de novo.";

function mensagemDoErro(erro: unknown, padrao: string): string {
  return typeof erro === "string" ? erro : padrao;
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
  }, []);

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

  const alertaDeDados = erroDeDados && (
    <p role="alert" className="erro">
      {erroDeDados}
    </p>
  );

  // Erro ao verificar o Ollama: não dá para seguir, então só mostra a mensagem.
  if (erroDoOllama) {
    return (
      <main className="app">
        <h1>Memoteca</h1>
        {alertaDeDados}
        <p role="alert" className="erro">
          {erroDoOllama}
        </p>
      </main>
    );
  }

  const carregando = ollama === null || onboardingConcluido === null;
  const precisaDeOnboarding =
    !carregando &&
    (!onboardingConcluido ||
      ollama.estado !== "em_execucao" ||
      (ollama.modelosInstalados ?? 0) === 0);

  if (precisaDeOnboarding) {
    return (
      <main className="app">
        <h1>Memoteca</h1>
        {alertaDeDados}
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
    <main className="app">
      <h1>Memoteca</h1>
      {alertaDeDados ?? <p>Em construção.</p>}
      {ollama?.estado === "em_execucao" && (
        <>
          <p role="status">Ollama {ollama.versao} em execução.</p>
          {ollama.aviso && (
            <p role="status" className="aviso">
              {ollama.aviso}
            </p>
          )}
        </>
      )}
    </main>
  );
}

export default App;
