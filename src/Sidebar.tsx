import { useProjetos } from "./estado/projetos";
import type { EstadoOllama } from "./tipos";

interface Props {
  ollama: EstadoOllama;
}

/**
 * Barra lateral: lista de projetos e o estado do Ollama. As conversas aparecem dentro de cada
 * projeto (próxima etapa). Não há "nova conversa" aqui: toda conversa pertence a um projeto.
 */
function Sidebar({ ollama }: Props) {
  const { projetos, selecionadoId, selecionar, iniciarNovo } = useProjetos();

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
          aria-label="Novo projeto"
          className="rounded px-2 py-0.5 text-lg leading-none hover:bg-neutral-200 dark:hover:bg-neutral-800"
        >
          +
        </button>
      </div>

      <nav aria-label="Lista de projetos" className="flex-1 overflow-y-auto">
        <ul className="space-y-0.5">
          {projetos.map((projeto) => (
            <li key={projeto.id}>
              <button
                type="button"
                onClick={() => selecionar(projeto.id)}
                aria-current={projeto.id === selecionadoId ? "true" : undefined}
                className="w-full truncate rounded px-2 py-1 text-left hover:bg-neutral-200 aria-[current=true]:bg-neutral-200 aria-[current=true]:font-medium dark:hover:bg-neutral-800 dark:aria-[current=true]:bg-neutral-800"
              >
                {projeto.nome}
              </button>
            </li>
          ))}
        </ul>
      </nav>

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
