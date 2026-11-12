import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { AVISO_DE_INDEPENDENCIA, REPOSITORIO } from "./avisoDeIndependencia";

/**
 * Tela "Sobre" (T09): versão, licença, repositório e o aviso de independência em relação ao
 * Ollama. O endereço do repositório aparece como texto, sem link clicável: o app não abre
 * endereços externos (decisão registrada no PLANO.md).
 */
function TelaSobre() {
  const [versao, setVersao] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    getVersion()
      .then((valor) => {
        if (ativo) setVersao(valor);
      })
      .catch(() => {
        // Sem a versão a tela continua útil: o resto não depende dela.
      });
    return () => {
      ativo = false;
    };
  }, []);

  return (
    <article aria-label="Sobre o Memoteca" className="max-w-2xl space-y-4">
      <h2 className="text-xl font-semibold">Sobre o Memoteca</h2>

      <p>
        Aplicativo para organizar suas conversas e a memória de modelos de
        linguagem locais.
      </p>

      <section aria-label="Independência em relação ao Ollama">
        <h3 className="font-medium">Aviso de independência</h3>
        <p>{AVISO_DE_INDEPENDENCIA}</p>
      </section>

      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1">
        <dt className="font-medium">Versão</dt>
        <dd>{versao ?? "—"}</dd>
        <dt className="font-medium">Licença</dt>
        <dd>MIT, Memoteca contributors</dd>
        <dt className="font-medium">Repositório</dt>
        <dd className="select-text break-all">{REPOSITORIO}</dd>
      </dl>
    </article>
  );
}

export default TelaSobre;
