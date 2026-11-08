import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import InstalarOllama from "./InstalarOllama";

const MENSAGEM_DADOS =
  "Não foi possível verificar os dados do Memoteca. Feche o aplicativo e abra-o de novo.";
const MENSAGEM_OLLAMA =
  "Não foi possível verificar o Ollama. Feche o aplicativo e abra-o de novo.";

/** Estado do Ollama, como o backend devolve. */
interface EstadoOllama {
  estado: "em_execucao" | "nao_instalado" | "porta_ocupada" | "indisponivel";
  versao: string | null;
  origem: "externo" | "iniciado_pelo_app" | null;
  modelosInstalados: number | null;
  aviso: string | null;
  mensagem: string | null;
}

function mensagemDoErro(erro: unknown, padrao: string): string {
  return typeof erro === "string" ? erro : padrao;
}

function App() {
  const [erroDeDados, setErroDeDados] = useState<string | null>(null);
  const [ollama, setOllama] = useState<EstadoOllama | null>(null);
  const [erroDoOllama, setErroDoOllama] = useState<string | null>(null);

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

  const problemaComOllama =
    erroDoOllama ??
    (ollama && ollama.estado !== "em_execucao" ? ollama.mensagem : null);

  return (
    <main className="app">
      <h1>Memoteca</h1>
      {erroDeDados ? (
        <p role="alert" className="erro">
          {erroDeDados}
        </p>
      ) : (
        <p>Em construção.</p>
      )}
      {problemaComOllama && (
        <p role="alert" className="erro">
          {problemaComOllama}
        </p>
      )}
      {ollama?.estado === "nao_instalado" && (
        <InstalarOllama aoConcluir={prepararOllama} />
      )}
      {ollama?.estado === "em_execucao" && (
        <>
          <p role="status">Ollama {ollama.versao} em execução.</p>
          {ollama.aviso && (
            <p role="status" className="aviso">
              {ollama.aviso}
            </p>
          )}
          {ollama.modelosInstalados === 0 && (
            <p role="status">Nenhum modelo instalado ainda.</p>
          )}
        </>
      )}
    </main>
  );
}

export default App;
