import { useEffect, useRef, useState } from "react";
import ConfirmarExclusaoDeConversa from "./ConfirmarExclusaoDeConversa";
import { useConversas } from "./estado/conversas";
import MensagemNaTela, { RespostaChegando } from "./MensagemNaTela";
import ParametrosDaConversa from "./ParametrosDaConversa";
import {
  MENSAGEM_MAXIMA,
  TITULO_DA_CONVERSA_MAXIMO,
  type Conversa,
} from "./tipos";

function Titulo({ conversa }: { conversa: Conversa }) {
  const atualizarConversa = useConversas((estado) => estado.atualizarConversa);
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(conversa.titulo);
  const [erro, setErro] = useState<string | null>(null);

  async function salvar(evento: React.FormEvent) {
    evento.preventDefault();
    if (texto.trim() === "") {
      setErro("Dê um título à conversa.");
      return;
    }
    const falha = await atualizarConversa(conversa.id, {
      titulo: texto,
      modelo: conversa.modelo,
      temperature: conversa.temperature,
      numCtx: conversa.numCtx,
      keepAlive: conversa.keepAlive,
    });
    if (falha) {
      setErro(falha);
      return;
    }
    setEditando(false);
  }

  if (!editando) {
    return (
      <div className="flex items-center gap-2">
        <h2 className="truncate text-xl font-semibold">{conversa.titulo}</h2>
        <button
          type="button"
          onClick={() => {
            setTexto(conversa.titulo);
            setErro(null);
            setEditando(true);
          }}
          className="rounded border border-neutral-400 px-2 py-0.5 text-sm"
        >
          Renomear
        </button>
      </div>
    );
  }

  return (
    <form
      onSubmit={(evento) => void salvar(evento)}
      className="flex flex-wrap gap-2"
    >
      <label className="flex items-center gap-2">
        Título da conversa
        <input
          type="text"
          value={texto}
          maxLength={TITULO_DA_CONVERSA_MAXIMO}
          onChange={(evento) => setTexto(evento.target.value)}
          className="rounded border border-neutral-400 bg-transparent px-2 py-1"
        />
      </label>
      <button
        type="submit"
        className="rounded bg-blue-700 px-3 py-1 text-white"
      >
        Salvar título
      </button>
      <button
        type="button"
        onClick={() => setEditando(false)}
        className="rounded border border-neutral-400 px-3 py-1"
      >
        Cancelar
      </button>
      {erro && (
        <p role="alert" className="erro w-full">
          {erro}
        </p>
      )}
    </form>
  );
}

function Campo({ conversa }: { conversa: Conversa }) {
  const { gerando, enviar, cancelar } = useConversas();
  const [texto, setTexto] = useState("");
  const semModelo = !conversa.modelo;

  async function enviarTexto() {
    if (texto.trim() === "" || gerando || semModelo) {
      return;
    }
    const { persistiu } = await enviar(texto);
    // Se a mensagem não foi gravada (por exemplo, recusada), o texto fica para ser corrigido.
    if (persistiu) {
      setTexto("");
    }
  }

  return (
    <div className="space-y-2">
      {semModelo && (
        <p role="status" className="aviso">
          Escolha um modelo em &quot;Modelo e parâmetros&quot; para poder
          enviar.
        </p>
      )}
      <div className="flex gap-2">
        <textarea
          aria-label="Mensagem"
          value={texto}
          rows={3}
          maxLength={MENSAGEM_MAXIMA}
          placeholder="Escreva sua mensagem. Enter envia, Shift+Enter quebra a linha."
          onChange={(evento) => setTexto(evento.target.value)}
          onKeyDown={(evento) => {
            if (evento.key === "Enter" && !evento.shiftKey) {
              evento.preventDefault();
              void enviarTexto();
            }
          }}
          className="flex-1 rounded border border-neutral-400 bg-transparent px-2 py-1"
        />
        {gerando ? (
          <button
            type="button"
            onClick={cancelar}
            className="self-end rounded border border-red-700 px-3 py-1 text-red-700 dark:text-red-400"
          >
            Parar
          </button>
        ) : (
          <button
            type="button"
            onClick={() => void enviarTexto()}
            disabled={texto.trim() === "" || semModelo}
            className="self-end rounded bg-blue-700 px-3 py-1 text-white disabled:opacity-50"
          >
            Enviar
          </button>
        )}
      </div>
    </div>
  );
}

interface Props {
  conversa: Conversa;
}

/** A conversa ativa: título, parâmetros, mensagens com streaming e o campo de envio. */
function TelaDaConversa({ conversa }: Props) {
  const {
    mensagens,
    gerando,
    emAndamento,
    erroDeEnvio,
    respostaCortada,
    regenerar,
    tentarDeNovo,
    excluirConversa,
  } = useConversas();
  const [confirmandoExclusao, setConfirmandoExclusao] = useState(false);
  const fim = useRef<HTMLDivElement>(null);

  // Acompanha a resposta que está chegando.
  useEffect(() => {
    fim.current?.scrollIntoView?.({ block: "end" });
  }, [mensagens.length, emAndamento?.conteudo]);

  const ultima = mensagens[mensagens.length - 1];
  // Depois de uma falha, a última mensagem diz o que tentar de novo.
  const aguardandoResposta = ultima?.papel === "user";
  const podeTentarDeNovo =
    !gerando &&
    erroDeEnvio !== null &&
    (aguardandoResposta || ultima?.estado === "interrompida");

  return (
    <section
      aria-label="Conversa"
      className="flex h-full max-w-3xl flex-col gap-3"
    >
      <div className="flex items-center justify-between gap-2">
        <Titulo key={conversa.titulo} conversa={conversa} />
        <button
          type="button"
          disabled={gerando}
          onClick={() => setConfirmandoExclusao(true)}
          className="rounded border border-red-700 px-2 py-0.5 text-sm text-red-700 disabled:opacity-50 dark:text-red-400"
        >
          Excluir conversa
        </button>
      </div>

      <ParametrosDaConversa conversa={conversa} />

      <div
        aria-label="Mensagens"
        className="flex-1 space-y-4 overflow-y-auto rounded border border-neutral-300 p-3 dark:border-neutral-700"
      >
        {mensagens.length === 0 && !gerando && (
          <p className="text-neutral-600 dark:text-neutral-400">
            Escreva a primeira mensagem para começar.
          </p>
        )}
        {mensagens.map((mensagem) => (
          <MensagemNaTela
            key={mensagem.id}
            mensagem={mensagem}
            podeRegenerar={
              !gerando &&
              mensagem.papel === "assistant" &&
              mensagem.id === ultima?.id
            }
            aoRegenerar={() => void regenerar()}
          />
        ))}
        {gerando && emAndamento && (
          <RespostaChegando
            conteudo={emAndamento.conteudo}
            raciocinio={emAndamento.raciocinio}
          />
        )}
        {respostaCortada && (
          <p role="status" className="aviso text-sm">
            O modelo parou porque a resposta chegou ao limite de tamanho.
            Aumente o contexto nos parâmetros para respostas maiores.
          </p>
        )}
        {erroDeEnvio && (
          <div role="alert" className="erro space-y-1">
            <p>{erroDeEnvio}</p>
            {podeTentarDeNovo && (
              <button
                type="button"
                onClick={() =>
                  void (aguardandoResposta ? tentarDeNovo() : regenerar())
                }
                className="rounded border border-red-700 px-2 py-0.5 text-sm"
              >
                Tentar de novo
              </button>
            )}
          </div>
        )}
        <div ref={fim} />
      </div>

      <Campo conversa={conversa} />

      {confirmandoExclusao && (
        <ConfirmarExclusaoDeConversa
          conversa={conversa}
          aoConfirmar={() => excluirConversa(conversa.id)}
          aoCancelar={() => setConfirmandoExclusao(false)}
        />
      )}
    </section>
  );
}

export default TelaDaConversa;
