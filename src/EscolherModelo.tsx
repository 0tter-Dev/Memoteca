import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  formatarTamanho,
  type AvaliacaoDeEspaco,
  type Catalogo,
  type ModeloDoCatalogo,
} from "./tipos";

import { mensagemDoErro } from "./erros";

const MENSAGEM_PADRAO =
  "Não foi possível carregar a lista de modelos. Feche o aplicativo e abra-o de novo.";

const TITULO_DA_FAIXA: Record<ModeloDoCatalogo["faixa"], string> = {
  leve: "Leves (computadores simples)",
  equilibrado: "Equilibrados",
  potente: "Potentes (exigem muita memória e disco)",
};

const FAIXAS: ModeloDoCatalogo["faixa"][] = ["leve", "equilibrado", "potente"];

interface Props {
  /** Chamado com o nome do modelo quando o usuário confirma o download. */
  aoBaixar: (nome: string) => void;
}

/** Passo "Modelo" do onboarding: catálogo curado e campo livre. */
function EscolherModelo({ aoBaixar }: Props) {
  const [catalogo, setCatalogo] = useState<Catalogo | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [escolhido, setEscolhido] = useState<string | null>(null);
  const [outro, setOutro] = useState("");
  const [espaco, setEspaco] = useState<AvaliacaoDeEspaco | null>(null);
  const [aceitouAlerta, setAceitouAlerta] = useState(false);

  useEffect(() => {
    let ativo = true;
    invoke<Catalogo>("catalogo_de_modelos")
      .then((resposta) => {
        if (ativo) {
          setCatalogo(resposta);
        }
      })
      .catch((falha: unknown) => {
        if (ativo) {
          setErro(mensagemDoErro(falha, MENSAGEM_PADRAO));
        }
      });
    return () => {
      ativo = false;
    };
  }, []);

  function escolher(tag: string) {
    setEscolhido(tag);
    setOutro("");
    setEspaco(null);
    setAceitouAlerta(false);
    invoke<AvaliacaoDeEspaco>("avaliar_espaco_do_modelo", { tag })
      .then(setEspaco)
      .catch(() => setEspaco(null));
  }

  function digitar(texto: string) {
    setOutro(texto);
    setEscolhido(null);
    setEspaco(null);
    setAceitouAlerta(false);
  }

  const nomeFinal = escolhido ?? outro.trim();
  const bloqueado = espaco?.estado === "insuficiente";
  const precisaAceitar = espaco?.estado === "alerta" && !aceitouAlerta;
  const podeBaixar = nomeFinal !== "" && !bloqueado && !precisaAceitar;

  if (erro) {
    return (
      <p role="alert" className="erro">
        {erro}
      </p>
    );
  }
  if (!catalogo) {
    return <p role="status">Carregando a lista de modelos…</p>;
  }

  return (
    <section aria-label="Escolha do modelo">
      <h2>Escolha um modelo</h2>
      <p>
        O modelo é o &quot;cérebro&quot; que responde às suas conversas. Ele é
        baixado uma vez e fica no seu computador. Modelos maiores respondem
        melhor, mas pedem mais memória e espaço em disco.
      </p>

      {FAIXAS.map((faixa) => {
        const doGrupo = catalogo.modelos.filter((m) => m.faixa === faixa);
        if (doGrupo.length === 0) {
          return null;
        }
        return (
          <fieldset key={faixa}>
            <legend>{TITULO_DA_FAIXA[faixa]}</legend>
            {doGrupo.map((modelo) => (
              <label key={modelo.tag} className="modelo">
                <input
                  type="radio"
                  name="modelo"
                  value={modelo.tag}
                  checked={escolhido === modelo.tag}
                  onChange={() => escolher(modelo.tag)}
                />{" "}
                <strong>{modelo.nome}</strong>
                {modelo.raciocinio ? " · com raciocínio" : ""}
                <br />
                <small>
                  {modelo.descricao} {formatarTamanho(modelo.tamanhoEmBytes)} de
                  download, cerca de {modelo.ramEstimadaEmGb} GB de memória
                  (estimativa).
                </small>
              </label>
            ))}
          </fieldset>
        );
      })}

      <label>
        Outro modelo (nome da biblioteca do Ollama, por exemplo{" "}
        <code>llama3.2:1b</code>)
        <br />
        <input
          type="text"
          value={outro}
          onChange={(evento) => digitar(evento.target.value)}
          placeholder="nome ou nome:tag"
        />
      </label>
      {outro.trim() !== "" && (
        <p role="note">
          O tamanho só é conhecido quando o download começa. Se não couber no
          disco, ele é interrompido.
        </p>
      )}

      {espaco?.mensagem && (
        <p
          role={bloqueado ? "alert" : "status"}
          className={bloqueado ? "erro" : "aviso"}
        >
          {espaco.mensagem}
        </p>
      )}
      {espaco?.estado === "alerta" && (
        <label>
          <input
            type="checkbox"
            checked={aceitouAlerta}
            onChange={(evento) => setAceitouAlerta(evento.target.checked)}
          />{" "}
          Quero baixar mesmo assim
        </label>
      )}

      <p>
        <button
          type="button"
          disabled={!podeBaixar}
          onClick={() => aoBaixar(nomeFinal)}
        >
          Baixar
        </button>
      </p>
    </section>
  );
}

export default EscolherModelo;
