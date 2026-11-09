import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useConversas } from "./estado/conversas";
import type { Conversa, DetalhesDoModelo } from "./tipos";

const CONTEXTOS = [2048, 4096, 8192, 16384, 32768, 65536, 131072];

const TEMPOS_NA_MEMORIA: [valor: string, rotulo: string][] = [
  ["5m", "5 minutos"],
  ["30m", "30 minutos"],
  ["1h", "1 hora"],
  ["24h", "24 horas"],
  ["-1", "Manter carregado"],
  ["0", "Descarregar logo"],
];

const CLASSE_DO_CAMPO =
  "mt-1 w-full rounded border border-neutral-400 bg-transparent px-2 py-1";

interface Props {
  conversa: Conversa;
}

/** Modelo e parâmetros (`temperature`, `num_ctx`, `keep_alive`) da conversa. Cada conversa tem os seus. */
function ParametrosDaConversa({ conversa }: Props) {
  const atualizarConversa = useConversas((estado) => estado.atualizarConversa);
  const gerando = useConversas((estado) => estado.gerando);
  const [instalados, setInstalados] = useState<string[]>([]);
  const [janela, setJanela] = useState<number | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    invoke<string[]>("listar_modelos_instalados")
      .then((nomes) => {
        if (ativo) {
          setInstalados(nomes);
        }
      })
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, []);

  // A janela de contexto do modelo limita o `num_ctx` oferecido.
  useEffect(() => {
    let ativo = true;
    if (!conversa.modelo) {
      return;
    }
    invoke<DetalhesDoModelo>("detalhes_do_modelo", { nome: conversa.modelo })
      .then((detalhes) => {
        if (ativo) {
          setJanela(detalhes.janelaDeContexto);
        }
      })
      .catch(() => {
        if (ativo) {
          setJanela(null);
        }
      });
    return () => {
      ativo = false;
    };
  }, [conversa.modelo]);

  async function salvar(parcial: Partial<Conversa>) {
    setErro(null);
    const falha = await atualizarConversa(conversa.id, {
      titulo: conversa.titulo,
      modelo: conversa.modelo,
      temperature: conversa.temperature,
      numCtx: conversa.numCtx,
      keepAlive: conversa.keepAlive,
      ...parcial,
    });
    if (falha) {
      setErro(falha);
    }
  }

  const modelos = [...instalados];
  if (conversa.modelo && !modelos.includes(conversa.modelo)) {
    modelos.unshift(conversa.modelo);
  }
  const contextos = CONTEXTOS.filter(
    (valor) => janela === null || valor <= janela,
  );
  if (conversa.numCtx !== null && !contextos.includes(conversa.numCtx)) {
    contextos.push(conversa.numCtx);
    contextos.sort((a, b) => a - b);
  }

  return (
    <details className="rounded border border-neutral-300 p-2 dark:border-neutral-700">
      <summary className="cursor-pointer">Modelo e parâmetros</summary>
      <div className="mt-2 grid gap-3 sm:grid-cols-2">
        <label className="block">
          Modelo
          <select
            value={conversa.modelo ?? ""}
            disabled={gerando}
            onChange={(evento) =>
              void salvar({ modelo: evento.target.value || null })
            }
            className={CLASSE_DO_CAMPO}
          >
            <option value="">Escolha um modelo</option>
            {modelos.map((nome) => (
              <option key={nome} value={nome}>
                {nome}
              </option>
            ))}
          </select>
        </label>

        <label className="block">
          Temperatura (0 a 2)
          <input
            type="number"
            min={0}
            max={2}
            step={0.1}
            value={conversa.temperature ?? ""}
            disabled={gerando}
            onChange={(evento) =>
              void salvar({
                temperature:
                  evento.target.value === ""
                    ? null
                    : Number(evento.target.value),
              })
            }
            className={CLASSE_DO_CAMPO}
          />
        </label>

        <label className="block">
          Contexto (tokens)
          <select
            value={conversa.numCtx ?? ""}
            disabled={gerando}
            onChange={(evento) =>
              void salvar({
                numCtx:
                  evento.target.value === ""
                    ? null
                    : Number(evento.target.value),
              })
            }
            className={CLASSE_DO_CAMPO}
          >
            <option value="">Padrão do modelo</option>
            {contextos.map((valor) => (
              <option key={valor} value={valor}>
                {valor.toLocaleString("pt-BR")}
              </option>
            ))}
          </select>
        </label>

        <label className="block">
          Manter o modelo na memória
          <select
            value={conversa.keepAlive ?? ""}
            disabled={gerando}
            onChange={(evento) =>
              void salvar({ keepAlive: evento.target.value || null })
            }
            className={CLASSE_DO_CAMPO}
          >
            <option value="">Padrão do Ollama</option>
            {TEMPOS_NA_MEMORIA.map(([valor, rotulo]) => (
              <option key={valor} value={valor}>
                {rotulo}
              </option>
            ))}
          </select>
        </label>
      </div>
      {erro && (
        <p role="alert" className="erro mt-2">
          {erro}
        </p>
      )}
    </details>
  );
}

export default ParametrosDaConversa;
