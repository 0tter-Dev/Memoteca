import { useEffect, useState } from "react";
import { useConversas } from "./estado/conversas";
import { useProjetos } from "./estado/projetos";
import TelaDaConversa from "./TelaDaConversa";
import TelaDeMemorias from "./TelaDeMemorias";
import FormularioDeProjeto from "./FormularioDeProjeto";
import Sidebar from "./Sidebar";
import type { Conversa, EstadoOllama, Projeto } from "./tipos";

interface Props {
  ollama: EstadoOllama;
  /** Mensagem amigável quando o banco de dados não abriu. */
  erroDeDados: string | null;
}

/** Lista vazia constante: um seletor do Zustand não pode devolver um array novo a cada leitura. */
const SEM_CONVERSAS: Conversa[] = [];

function DetalheDoProjeto({ projeto }: { projeto: Projeto }) {
  const iniciarEdicao = useProjetos((estado) => estado.iniciarEdicao);
  const abrirMemorias = useProjetos((estado) => estado.abrirMemorias);
  const criarConversa = useConversas((estado) => estado.criarConversa);
  const [erro, setErro] = useState<string | null>(null);

  async function novaConversa() {
    setErro(null);
    const falha = await criarConversa(projeto.id);
    if (falha) {
      setErro(falha);
    }
  }

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

      <section>
        <h3 className="font-medium">Memórias do projeto</h3>
        <p className="text-neutral-600 dark:text-neutral-400">
          Fatos que o modelo lembra em toda conversa deste projeto.
        </p>
        <button
          type="button"
          onClick={() => abrirMemorias("projeto")}
          className="mt-1 rounded border border-neutral-400 px-3 py-1"
        >
          Gerenciar memórias
        </button>
      </section>

      <div className="space-y-2">
        <button
          type="button"
          onClick={() => void novaConversa()}
          className="rounded bg-blue-700 px-3 py-1 text-white"
        >
          Nova conversa
        </button>
        <p className="text-neutral-600 dark:text-neutral-400">
          As conversas do projeto aparecem na lateral.
        </p>
        {erro && (
          <p role="alert" className="erro">
            {erro}
          </p>
        )}
      </div>
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
  const ativaId = useConversas((estado) => estado.ativaId);
  const conversasDoProjeto = useConversas((estado) =>
    selecionadoId
      ? (estado.conversasPorProjeto[selecionadoId] ?? SEM_CONVERSAS)
      : SEM_CONVERSAS,
  );
  const conversaAtiva =
    conversasDoProjeto.find((c) => c.id === ativaId) ?? null;

  let conteudo: React.ReactNode;
  if (erro) {
    conteudo = (
      <p role="alert" className="erro">
        {erro}
      </p>
    );
  } else if (!carregado) {
    conteudo = <p role="status">Carregando os projetos…</p>;
  } else if (modo === "memorias") {
    conteudo = <TelaDeMemorias projeto={selecionado} />;
  } else if (modo === "novo") {
    conteudo = <FormularioDeProjeto key="novo" projeto={null} />;
  } else if (modo === "ver" && conversaAtiva) {
    conteudo = (
      <TelaDaConversa key={conversaAtiva.id} conversa={conversaAtiva} />
    );
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
