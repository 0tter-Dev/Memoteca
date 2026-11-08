import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

const MENSAGEM_PADRAO =
  "Não foi possível verificar os dados do Memoteca. Feche o aplicativo e abra-o de novo.";

function App() {
  const [erroDeDados, setErroDeDados] = useState<string | null>(null);

  // O backend abre o banco ao iniciar. Se falhar, ele devolve a mensagem amigável.
  useEffect(() => {
    let ativo = true;
    invoke("estado_do_banco").catch((erro: unknown) => {
      if (ativo) {
        setErroDeDados(typeof erro === "string" ? erro : MENSAGEM_PADRAO);
      }
    });
    return () => {
      ativo = false;
    };
  }, []);

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
    </main>
  );
}

export default App;
