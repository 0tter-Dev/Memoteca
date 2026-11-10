import { useEffect, useState } from "react";
import { useMemorias } from "./estado/memorias";
import { useProjetos } from "./estado/projetos";
import { MEMORIA_MAXIMA } from "./tipos";
import type { EscopoDeMemoria, Memoria, Projeto } from "./tipos";

const SEM_MEMORIAS: Memoria[] = [];

const CLASSE_DO_CAMPO =
  "mt-1 w-full rounded border border-neutral-400 bg-transparent px-2 py-1";

function resumo(texto: string): string {
  const linha = texto.trim().split("\n")[0] ?? "";
  return linha.length > 40 ? `${linha.slice(0, 40)}…` : linha;
}

interface CampoProps {
  rotulo: string;
  valor: string;
  aoMudar: (valor: string) => void;
}

/** Campo de texto de uma memória, com o contador de caracteres. */
function CampoDeMemoria({ rotulo, valor, aoMudar }: CampoProps) {
  return (
    <div>
      <label className="block">
        {rotulo}
        <textarea
          value={valor}
          onChange={(evento) => aoMudar(evento.target.value)}
          rows={2}
          className={CLASSE_DO_CAMPO}
        />
      </label>
      <span
        className={`text-xs ${
          valor.length > MEMORIA_MAXIMA
            ? "text-red-700 dark:text-red-400"
            : "text-neutral-600 dark:text-neutral-400"
        }`}
      >
        {valor.length} de {MEMORIA_MAXIMA} caracteres
      </span>
    </div>
  );
}

function ItemDeMemoria({ memoria }: { memoria: Memoria }) {
  const atualizar = useMemorias((estado) => estado.atualizar);
  const excluir = useMemorias((estado) => estado.excluir);
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(memoria.conteudo);
  const [erro, setErro] = useState<string | null>(null);

  async function salvar() {
    setErro(null);
    const falha = await atualizar(memoria, texto);
    if (falha) {
      setErro(falha);
    } else {
      setEditando(false);
    }
  }

  async function remover() {
    setErro(null);
    const falha = await excluir(memoria);
    if (falha) {
      setErro(falha);
    }
  }

  return (
    <li className="space-y-2 rounded border border-neutral-300 p-2 dark:border-neutral-700">
      {editando ? (
        <>
          <CampoDeMemoria
            rotulo="Texto da memória"
            valor={texto}
            aoMudar={setTexto}
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void salvar()}
              disabled={texto.trim() === ""}
              className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-60"
            >
              Salvar
            </button>
            <button
              type="button"
              onClick={() => {
                setEditando(false);
                setTexto(memoria.conteudo);
                setErro(null);
              }}
              className="rounded border border-neutral-400 px-3 py-1"
            >
              Cancelar
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="whitespace-pre-wrap">{memoria.conteudo}</p>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => {
                setTexto(memoria.conteudo);
                setEditando(true);
              }}
              aria-label={`Editar a memória “${resumo(memoria.conteudo)}”`}
              className="rounded border border-neutral-400 px-2 py-0.5 text-sm"
            >
              Editar
            </button>
            <button
              type="button"
              onClick={() => void remover()}
              aria-label={`Remover a memória “${resumo(memoria.conteudo)}”`}
              className="rounded border border-red-700 px-2 py-0.5 text-sm text-red-700 dark:text-red-400"
            >
              Remover
            </button>
          </div>
        </>
      )}
      {erro && (
        <p role="alert" className="erro text-sm">
          {erro}
        </p>
      )}
    </li>
  );
}

interface ListaProps {
  escopo: EscopoDeMemoria;
  projetoId: string | null;
  memorias: Memoria[];
}

/** As memórias de um escopo e o campo para criar uma nova. */
function ListaDeMemorias({ escopo, projetoId, memorias }: ListaProps) {
  const criar = useMemorias((estado) => estado.criar);
  const [texto, setTexto] = useState("");
  const [erro, setErro] = useState<string | null>(null);

  async function adicionar() {
    setErro(null);
    const falha = await criar(escopo, projetoId, texto);
    if (falha) {
      setErro(falha);
    } else {
      setTexto("");
    }
  }

  return (
    <div className="space-y-3">
      <p className="text-neutral-600 dark:text-neutral-400">
        {escopo === "global"
          ? "As memórias globais vão em toda conversa, de qualquer projeto."
          : "As memórias do projeto vão só nas conversas deste projeto."}{" "}
        Elas valem para as próximas mensagens que você enviar. As respostas já
        geradas não mudam.
      </p>

      {memorias.length === 0 ? (
        <p>Nenhuma memória ainda.</p>
      ) : (
        <ul aria-label="Memórias" className="space-y-2">
          {memorias.map((memoria) => (
            <ItemDeMemoria key={memoria.id} memoria={memoria} />
          ))}
        </ul>
      )}

      <form
        onSubmit={(evento) => {
          evento.preventDefault();
          void adicionar();
        }}
        className="space-y-2"
      >
        <CampoDeMemoria
          rotulo="Nova memória"
          valor={texto}
          aoMudar={setTexto}
        />
        <button
          type="submit"
          disabled={texto.trim() === ""}
          className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-60"
        >
          Adicionar memória
        </button>
        {erro && (
          <p role="alert" className="erro text-sm">
            {erro}
          </p>
        )}
      </form>
    </div>
  );
}

interface Props {
  /** O projeto aberto, se houver. Sem projeto, só existem as memórias globais. */
  projeto: Projeto | null;
}

const ABA =
  "rounded-t border border-b-0 border-neutral-400 px-3 py-1 aria-selected:bg-neutral-200 aria-selected:font-medium dark:aria-selected:bg-neutral-800";

/** Tela dedicada às memórias: abas "Deste projeto" e "Globais". */
function TelaDeMemorias({ projeto }: Props) {
  const abaPedida = useProjetos((estado) => estado.abaDeMemorias);
  const abrirMemorias = useProjetos((estado) => estado.abrirMemorias);
  const voltar = useProjetos((estado) => estado.cancelarFormulario);
  const carregarGlobais = useMemorias((estado) => estado.carregarGlobais);
  const carregarDoProjeto = useMemorias((estado) => estado.carregarDoProjeto);
  const globais = useMemorias((estado) => estado.globais);
  const doProjeto = useMemorias((estado) =>
    projeto ? (estado.doProjeto[projeto.id] ?? SEM_MEMORIAS) : SEM_MEMORIAS,
  );
  const erro = useMemorias((estado) => estado.erro);

  const projetoId = projeto?.id ?? null;
  useEffect(() => {
    void carregarGlobais();
    if (projetoId) {
      void carregarDoProjeto(projetoId);
    }
  }, [projetoId, carregarGlobais, carregarDoProjeto]);

  // A aba vem do estado, então "Memória global" na lateral também troca a aba com a tela aberta.
  const abaAtual: EscopoDeMemoria =
    projeto && abaPedida === "projeto" ? "projeto" : "global";

  return (
    <section aria-label="Memórias" className="max-w-2xl space-y-3">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">Memórias</h2>
        <button
          type="button"
          onClick={voltar}
          className="rounded border border-neutral-400 px-3 py-1"
        >
          Voltar
        </button>
      </div>

      {erro && (
        <p role="alert" className="erro">
          {erro}
        </p>
      )}

      <div
        role="tablist"
        aria-label="Escopo das memórias"
        className="flex gap-1 border-b border-neutral-400"
      >
        {projeto && (
          <button
            type="button"
            role="tab"
            aria-selected={abaAtual === "projeto"}
            onClick={() => abrirMemorias("projeto")}
            className={ABA}
          >
            Deste projeto
          </button>
        )}
        <button
          type="button"
          role="tab"
          aria-selected={abaAtual === "global"}
          onClick={() => abrirMemorias("global")}
          className={ABA}
        >
          Globais
        </button>
      </div>

      <div role="tabpanel">
        {abaAtual === "projeto" && projeto ? (
          <ListaDeMemorias
            key={projeto.id}
            escopo="projeto"
            projetoId={projeto.id}
            memorias={doProjeto}
          />
        ) : (
          <ListaDeMemorias
            key="global"
            escopo="global"
            projetoId={null}
            memorias={globais}
          />
        )}
      </div>
    </section>
  );
}

export default TelaDeMemorias;
