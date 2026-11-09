import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import ConfirmarExclusao from "./ConfirmarExclusao";
import { useProjetos } from "./estado/projetos";
import {
  INSTRUCOES_DO_PROJETO_MAXIMO,
  NOME_DO_PROJETO_MAXIMO,
  type Projeto,
} from "./tipos";

interface Props {
  /** O projeto que está sendo editado, ou `null` para criar um novo. */
  projeto: Projeto | null;
}

const CLASSE_DO_CAMPO =
  "mt-1 w-full rounded border border-neutral-400 bg-transparent px-2 py-1";

/** Criar e editar um projeto: nome, instruções (system prompt) e modelo padrão. */
function FormularioDeProjeto({ projeto }: Props) {
  const { salvar, excluir, cancelarFormulario } = useProjetos();
  const [nome, setNome] = useState(projeto?.nome ?? "");
  const [instrucoes, setInstrucoes] = useState(projeto?.systemPrompt ?? "");
  const [modelo, setModelo] = useState(projeto?.modeloPadrao ?? "");
  const [instalados, setInstalados] = useState<string[] | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [salvando, setSalvando] = useState(false);
  const [confirmandoExclusao, setConfirmandoExclusao] = useState(false);

  useEffect(() => {
    let ativo = true;
    invoke<string[]>("listar_modelos_instalados")
      .then((nomes) => {
        if (ativo) {
          setInstalados(nomes);
        }
      })
      // Sem a lista, o modelo padrão fica como está (ou em branco).
      .catch(() => {
        if (ativo) {
          setInstalados([]);
        }
      });
    return () => {
      ativo = false;
    };
  }, []);

  // O modelo já salvo continua escolhível mesmo que não esteja mais instalado.
  const opcoes = [...(instalados ?? [])];
  if (modelo !== "" && !opcoes.includes(modelo)) {
    opcoes.unshift(modelo);
  }

  async function enviar(evento: React.FormEvent) {
    evento.preventDefault();
    if (nome.trim() === "") {
      setErro("Dê um nome ao projeto.");
      return;
    }
    setErro(null);
    setSalvando(true);
    const falha = await salvar({
      nome,
      systemPrompt: instrucoes,
      modeloPadrao: modelo === "" ? null : modelo,
    });
    setSalvando(false);
    if (falha) {
      setErro(falha);
    }
  }

  return (
    <form
      aria-label={projeto ? "Editar projeto" : "Novo projeto"}
      onSubmit={(evento) => void enviar(evento)}
      className="max-w-2xl space-y-4"
    >
      <h2 className="text-xl font-semibold">
        {projeto ? "Editar projeto" : "Novo projeto"}
      </h2>

      <label className="block">
        Nome do projeto
        <input
          type="text"
          value={nome}
          maxLength={NOME_DO_PROJETO_MAXIMO}
          onChange={(evento) => setNome(evento.target.value)}
          className={CLASSE_DO_CAMPO}
        />
      </label>

      <label className="block">
        Instruções do projeto
        <span className="block text-sm text-neutral-600 dark:text-neutral-400">
          O modelo recebe este texto em todas as conversas do projeto. Opcional.
        </span>
        <textarea
          value={instrucoes}
          rows={8}
          maxLength={INSTRUCOES_DO_PROJETO_MAXIMO}
          onChange={(evento) => setInstrucoes(evento.target.value)}
          className={CLASSE_DO_CAMPO}
        />
      </label>

      <label className="block">
        Modelo padrão
        <span className="block text-sm text-neutral-600 dark:text-neutral-400">
          As novas conversas do projeto começam com este modelo. Sem um padrão,
          vale o modelo da primeira conversa criada no projeto.
        </span>
        <select
          value={modelo}
          onChange={(evento) => setModelo(evento.target.value)}
          className={CLASSE_DO_CAMPO}
        >
          <option value="">Usar o modelo da primeira conversa</option>
          {opcoes.map((opcao) => (
            <option key={opcao} value={opcao}>
              {opcao}
            </option>
          ))}
        </select>
      </label>

      {erro && (
        <p role="alert" className="erro">
          {erro}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <button
          type="submit"
          disabled={salvando}
          className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-50"
        >
          {projeto ? "Salvar alterações" : "Criar projeto"}
        </button>
        <button
          type="button"
          onClick={cancelarFormulario}
          className="rounded border border-neutral-400 px-3 py-1"
        >
          Cancelar
        </button>
        {projeto && (
          <button
            type="button"
            onClick={() => setConfirmandoExclusao(true)}
            className="ml-auto rounded border border-red-700 px-3 py-1 text-red-700 dark:text-red-400"
          >
            Excluir projeto
          </button>
        )}
      </div>

      {projeto && confirmandoExclusao && (
        <ConfirmarExclusao
          projeto={projeto}
          aoConfirmar={() => excluir(projeto.id)}
          aoCancelar={() => setConfirmandoExclusao(false)}
        />
      )}
    </form>
  );
}

export default FormularioDeProjeto;
