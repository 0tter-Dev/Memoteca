import { useEffect, useState } from "react";
import { useConversas } from "./estado/conversas";
import { useProjetos } from "./estado/projetos";
import type { EstadoOllama } from "./tipos";

interface Props {
  ollama: EstadoOllama;
}

const BOTAO =
  "w-full truncate rounded px-2 py-1 text-left hover:bg-neutral-200 aria-[current=true]:bg-neutral-200 aria-[current=true]:font-medium disabled:opacity-60 dark:hover:bg-neutral-800 dark:aria-[current=true]:bg-neutral-800";

/**
 * Barra lateral: projetos e, dentro do projeto aberto, as conversas dele. A "Nova conversa"
 * só existe **dentro de um projeto**: toda conversa pertence a exatamente um projeto.
 */
function Sidebar({ ollama }: Props) {
  const {
    projetos,
    selecionadoId,
    modo,
    selecionar,
    iniciarNovo,
    abrirMemorias,
    abrirConfiguracoes,
  } = useProjetos();
  const {
    conversasPorProjeto,
    ativaId,
    gerando,
    carregarConversas,
    criarConversa,
    abrirConversa,
    fecharConversa,
  } = useConversas();
  const [erro, setErro] = useState<string | null>(null);
  const naTelaDeMemorias = modo === "memorias";
  const naTelaDeConfiguracoes = modo === "configuracoes";
  const emTelaAvulsa = naTelaDeMemorias || naTelaDeConfiguracoes;

  // As conversas do projeto aberto.
  useEffect(() => {
    if (selecionadoId) {
      void carregarConversas(selecionadoId);
    }
  }, [selecionadoId, carregarConversas]);

  async function novaConversa(projetoId: string) {
    setErro(null);
    selecionar(projetoId);
    const falha = await criarConversa(projetoId);
    if (falha) {
      setErro(falha);
    }
  }

  return (
    <aside
      aria-label="Projetos"
      className="flex w-64 shrink-0 flex-col border-r border-neutral-300 p-3 dark:border-neutral-700"
    >
      <h1 className="mb-3 text-lg font-semibold">Memoteca</h1>

      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-sm font-medium text-neutral-600 dark:text-neutral-400">
          Projetos
        </h2>
        <button
          type="button"
          onClick={iniciarNovo}
          disabled={gerando}
          aria-label="Novo projeto"
          className="rounded px-2 py-0.5 text-lg leading-none hover:bg-neutral-200 disabled:opacity-60 dark:hover:bg-neutral-800"
        >
          +
        </button>
      </div>

      <nav aria-label="Lista de projetos" className="flex-1 overflow-y-auto">
        <ul className="space-y-0.5">
          {projetos.map((projeto) => {
            const aberto = projeto.id === selecionadoId;
            const conversas = conversasPorProjeto[projeto.id] ?? [];
            return (
              <li key={projeto.id}>
                <button
                  type="button"
                  disabled={gerando}
                  onClick={() => {
                    selecionar(projeto.id);
                    fecharConversa();
                  }}
                  aria-current={
                    aberto && ativaId === null && !emTelaAvulsa
                      ? "true"
                      : undefined
                  }
                  aria-expanded={aberto}
                  className={BOTAO}
                >
                  {projeto.nome}
                </button>
                {aberto && (
                  <ul
                    aria-label={`Conversas de ${projeto.nome}`}
                    className="mt-0.5 ml-3 space-y-0.5 border-l border-neutral-300 pl-2 dark:border-neutral-700"
                  >
                    {conversas.map((conversa) => (
                      <li key={conversa.id}>
                        <button
                          type="button"
                          disabled={gerando}
                          onClick={() => {
                            selecionar(projeto.id);
                            void abrirConversa(conversa.id);
                          }}
                          aria-current={
                            conversa.id === ativaId && !emTelaAvulsa
                              ? "true"
                              : undefined
                          }
                          title={conversa.titulo}
                          className={BOTAO}
                        >
                          {conversa.titulo}
                        </button>
                      </li>
                    ))}
                    <li>
                      <button
                        type="button"
                        disabled={gerando}
                        onClick={() => void novaConversa(projeto.id)}
                        className="w-full rounded px-2 py-1 text-left text-sm text-blue-700 hover:bg-neutral-200 disabled:opacity-60 dark:text-blue-400 dark:hover:bg-neutral-800"
                      >
                        + Nova conversa
                      </button>
                    </li>
                  </ul>
                )}
              </li>
            );
          })}
        </ul>
        {erro && (
          <p role="alert" className="erro mt-2 text-sm">
            {erro}
          </p>
        )}
      </nav>

      <button
        type="button"
        disabled={gerando}
        onClick={() => abrirMemorias("global")}
        aria-current={naTelaDeMemorias ? "true" : undefined}
        className={`${BOTAO} mt-2`}
      >
        ◇ Memória global
      </button>
      <button
        type="button"
        disabled={gerando}
        onClick={abrirConfiguracoes}
        aria-current={naTelaDeConfiguracoes ? "true" : undefined}
        className={BOTAO}
      >
        ⚙ Configurações
      </button>

      <footer className="mt-3 text-xs text-neutral-600 dark:text-neutral-400">
        <p role="status">Ollama {ollama.versao} em execução.</p>
        {ollama.aviso && (
          <p role="status" className="aviso">
            {ollama.aviso}
          </p>
        )}
      </footer>
    </aside>
  );
}

export default Sidebar;
