import { useEffect } from "react";
import { useProjetos } from "./estado/projetos";
import FormularioDeProjeto from "./FormularioDeProjeto";
import Sidebar from "./Sidebar";
import type { EstadoOllama, Projeto } from "./tipos";

interface Props {
  ollama: EstadoOllama;
  /** Mensagem amigável quando o banco de dados não abriu. */
  erroDeDados: string | null;
}

function DetalheDoProjeto({ projeto }: { projeto: Projeto }) {
  const iniciarEdicao = useProjetos((estado) => estado.iniciarEdicao);

  return (
    <article aria-label="Projeto" className="max-w-2xl space-y-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">{projeto.nome}</h2>
        <button
          type="button"
          onClick={iniciarEdicao}
          className="rounded border border-neutral-400 px-3 py-1"
        >
          Editar
        </button>
      </div>

      <section>
        <h3 className="font-medium">Instruções do projeto</h3>
        {projeto.systemPrompt.trim() === "" ? (
          <p className="text-neutral-600 dark:text-neutral-400">
            Nenhuma instrução definida.
          </p>
        ) : (
          <p className="whitespace-pre-wrap">{projeto.systemPrompt}</p>
        )}
      </section>

      <section>
        <h3 className="font-medium">Modelo padrão</h3>
        <p>
          {projeto.modeloPadrao ??
            "Não definido: vale o modelo da primeira conversa criada no projeto."}
        </p>
      </section>

      <p className="text-neutral-600 dark:text-neutral-400">
        As conversas deste projeto aparecerão aqui em breve.
      </p>
    </article>
  );
}

/** Tela principal: projetos na lateral e o projeto escolhido (ou o formulário) no centro. */
function TelaPrincipal({ ollama, erroDeDados }: Props) {
  const {
    projetos,
    carregado,
    erro,
    selecionadoId,
    modo,
    carregar,
    iniciarNovo,
  } = useProjetos();

  useEffect(() => {
    void carregar();
  }, [carregar]);

  const selecionado = projetos.find((p) => p.id === selecionadoId) ?? null;

  let conteudo: React.ReactNode;
  if (erro) {
    conteudo = (
      <p role="alert" className="erro">
        {erro}
      </p>
    );
  } else if (!carregado) {
    conteudo = <p role="status">Carregando os projetos…</p>;
  } else if (modo === "novo") {
    conteudo = <FormularioDeProjeto key="novo" projeto={null} />;
  } else if (modo === "editar" && selecionado) {
    conteudo = (
      <FormularioDeProjeto key={selecionado.id} projeto={selecionado} />
    );
  } else if (selecionado) {
    conteudo = <DetalheDoProjeto projeto={selecionado} />;
  } else {
    conteudo = (
      <div className="max-w-md space-y-3">
        <h2 className="text-xl font-semibold">Nenhum projeto ainda</h2>
        <p>
          Os projetos organizam suas conversas e guardam instruções e memórias
          que valem para todas elas. Crie o primeiro para começar.
        </p>
        <button
          type="button"
          onClick={iniciarNovo}
          className="rounded bg-blue-700 px-3 py-1 text-white"
        >
          Criar o primeiro projeto
        </button>
      </div>
    );
  }

  return (
    <div className="flex h-screen">
      <Sidebar ollama={ollama} />
      <main className="flex-1 overflow-y-auto p-6">
        {erroDeDados && (
          <p role="alert" className="erro mb-4">
            {erroDeDados}
          </p>
        )}
        {conteudo}
      </main>
    </div>
  );
}

export default TelaPrincipal;
