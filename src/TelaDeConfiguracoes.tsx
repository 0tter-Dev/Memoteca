import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import ConfirmarImportacao from "./ConfirmarImportacao";
import { useConversas } from "./estado/conversas";
import { useProjetos } from "./estado/projetos";
import { mensagemDoErro } from "./erros";
import { descreverItens } from "./itensDeBackup";
import type {
  AnaliseDoBackup,
  ResultadoDaExportacao,
  ResumoDaImportacao,
} from "./tipos";

/** Tela de configurações. Por enquanto tem a seção "Dados": exportar e importar o backup. */
function TelaDeConfiguracoes() {
  const voltar = useProjetos((estado) => estado.cancelarFormulario);
  const gerando = useConversas((estado) => estado.gerando);
  const [ocupado, setOcupado] = useState(false);
  const [aviso, setAviso] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [analise, setAnalise] = useState<AnaliseDoBackup | null>(null);
  const [importando, setImportando] = useState(false);
  const [erroDaImportacao, setErroDaImportacao] = useState<string | null>(null);

  async function exportar() {
    setOcupado(true);
    setAviso(null);
    setErro(null);
    try {
      const resultado = await invoke<ResultadoDaExportacao>("exportar_backup");
      if (!resultado.cancelado && resultado.resumo) {
        setAviso(
          `Backup salvo em ${resultado.caminho}: ${descreverItens(resultado.resumo)}.`,
        );
      }
    } catch (falha: unknown) {
      setErro(mensagemDoErro(falha));
    } finally {
      setOcupado(false);
    }
  }

  async function escolherBackup() {
    setOcupado(true);
    setAviso(null);
    setErro(null);
    try {
      const resultado = await invoke<AnaliseDoBackup | null>("analisar_backup");
      if (resultado) {
        setErroDaImportacao(null);
        setAnalise(resultado);
      }
    } catch (falha: unknown) {
      setErro(mensagemDoErro(falha));
    } finally {
      setOcupado(false);
    }
  }

  function cancelarImportacao() {
    setAnalise(null);
    setErroDaImportacao(null);
    void Promise.resolve(invoke("descartar_backup_analisado")).catch(
      () => undefined,
    );
  }

  async function importar() {
    setImportando(true);
    setErroDaImportacao(null);
    try {
      const resumo = await invoke<ResumoDaImportacao>(
        "importar_backup_analisado",
      );
      setAnalise(null);
      setAviso(
        `Backup importado: ${descreverItens({
          projetos: resumo.projetos.novos,
          conversas: resumo.conversas.novos,
          mensagens: resumo.mensagens.novos,
          memorias: resumo.memorias.novos,
        })} adicionados.`,
      );
      // Os projetos e as conversas da lateral passam a incluir o que entrou.
      await useProjetos.getState().carregar();
      const aberto = useProjetos.getState().selecionadoId;
      if (aberto) {
        await useConversas.getState().carregarConversas(aberto);
      }
    } catch (falha: unknown) {
      setErroDaImportacao(mensagemDoErro(falha));
    } finally {
      setImportando(false);
    }
  }

  return (
    <section aria-label="Configurações" className="max-w-2xl space-y-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">Configurações</h2>
        <button
          type="button"
          onClick={voltar}
          className="rounded border border-neutral-400 px-3 py-1"
        >
          Voltar
        </button>
      </div>

      <section aria-labelledby="titulo-dados" className="space-y-3">
        <h3 id="titulo-dados" className="font-medium">
          Dados
        </h3>
        <p className="text-neutral-600 dark:text-neutral-400">
          O backup é um arquivo com todos os seus projetos, conversas, mensagens
          e memórias. Use-o para guardar uma cópia ou para levar seus dados a
          outro computador. O arquivo não é criptografado: guarde-o em um lugar
          seguro.
        </p>

        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={ocupado || gerando}
            onClick={() => void exportar()}
            className="rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-60"
          >
            Exportar backup…
          </button>
          <button
            type="button"
            disabled={ocupado || gerando}
            onClick={() => void escolherBackup()}
            className="rounded border border-neutral-400 px-3 py-1 disabled:opacity-60"
          >
            Importar backup…
          </button>
        </div>

        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          Importar nunca apaga nem altera o que você já tem: só adiciona o que
          ainda não existe aqui.
        </p>

        {aviso && <p role="status">{aviso}</p>}
        {erro && (
          <p role="alert" className="erro">
            {erro}
          </p>
        )}
      </section>

      {analise && (
        <ConfirmarImportacao
          analise={analise}
          importando={importando}
          erro={erroDaImportacao}
          aoConfirmar={() => void importar()}
          aoCancelar={cancelarImportacao}
        />
      )}
    </section>
  );
}

export default TelaDeConfiguracoes;
