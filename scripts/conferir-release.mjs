// Confere uma release do Memoteca já criada no GitHub (E17), antes de o mantenedor publicá-la.
//
// Uso: node scripts/conferir-release.mjs v0.1.0
//
// Precisa do `gh` (GitHub CLI) autenticado, dentro do repositório. Baixa o instalador anexado,
// calcula o SHA-256 e confere, nesta ordem:
//  - a tag tem o formato certo e a release tem um só instalador, com o nome da versão da tag;
//  - o SHA-256 escrito nas notas é o do arquivo baixado;
//  - as notas dizem que o instalador ainda não tem assinatura de código;
//  - nenhum texto da release afirma que o instalador é assinado (o instalador não é).
// Qualquer problema termina com código 1 e uma mensagem em português. Não altera a release.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const FORMATO_DA_TAG = /^v((0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*))$/;

/** Frases que afirmam que o instalador é assinado. */
const AFIRMACOES = [
  /instalador\s+(é\s+|foi\s+|está\s+|vem\s+)?assinado/i,
  /assinado\s+digitalmente/i,
  /assinatura\s+(de\s+código\s+)?(válida|verificada|confiável)/i,
  /publicador\s+verificado/i,
  /verified\s+publisher/i,
];

/** Uma frase com uma destas palavras nega ou adia a assinatura, e não a afirma. */
const NEGACAO = /\b(não|nem|sem|ainda|nunca|futuro|depois)\b/i;

/**
 * Linhas do texto que afirmam que o instalador é assinado. Linhas que negam ("ainda não tem
 * assinatura") não contam.
 */
export function afirmacoesDeAssinatura(texto) {
  return texto
    .split(/\r?\n/)
    .filter(
      (linha) =>
        AFIRMACOES.some((padrao) => padrao.test(linha)) && !NEGACAO.test(linha),
    )
    .map((linha) => linha.trim());
}

/** O texto avisa que o instalador ainda não tem assinatura de código. */
export function avisaQueNaoTemAssinatura(texto) {
  return /ainda não tem assinatura de código/i.test(texto);
}

/** O SHA-256 escrito nas notas da release, em minúsculas, ou `null`. */
export function shaDasNotas(corpo) {
  const achado = /SHA-256 do instalador:\*{0,2}\s*`([0-9a-fA-F]{64})`/.exec(
    corpo,
  );
  return achado ? achado[1].toLowerCase() : null;
}

/**
 * Problemas de uma release. `dados`: `tag`, `arquivos` (nomes dos anexos), `corpo` (as notas)
 * e `shaDoArquivo` (o SHA-256 do instalador baixado, em minúsculas). Lista vazia se está tudo certo.
 */
export function problemasDaRelease({ tag, arquivos, corpo, shaDoArquivo }) {
  const formato = FORMATO_DA_TAG.exec(tag ?? "");
  if (!formato) {
    return [
      `A tag "${tag ?? ""}" não tem o formato esperado. Use vMAJOR.MINOR.PATCH, por exemplo v0.1.0.`,
    ];
  }
  const problemas = [];

  const esperado = `Memoteca_${formato[1]}_x64-setup.exe`;
  const instaladores = arquivos.filter((nome) => nome.endsWith(".exe"));
  if (instaladores.length !== 1 || instaladores[0] !== esperado) {
    problemas.push(
      `A release deveria ter só o instalador ${esperado}, mas tem: ${arquivos.join(", ") || "nenhum anexo"}.`,
    );
  }

  const shaDasNotasDaRelease = shaDasNotas(corpo);
  if (!shaDasNotasDaRelease) {
    problemas.push("As notas da release não trazem o SHA-256 do instalador.");
  } else if (shaDasNotasDaRelease !== shaDoArquivo) {
    problemas.push(
      `O SHA-256 das notas (${shaDasNotasDaRelease}) não é o do arquivo baixado (${shaDoArquivo}).`,
    );
  }

  if (!avisaQueNaoTemAssinatura(corpo)) {
    problemas.push(
      'As notas deveriam avisar que o instalador "ainda não tem assinatura de código" (alerta do SmartScreen).',
    );
  }
  for (const linha of afirmacoesDeAssinatura(corpo)) {
    problemas.push(`As notas afirmam que o instalador é assinado: "${linha}".`);
  }
  return problemas;
}

function gh(argumentos) {
  // A saída de erro do `gh` fica fora da tela: quem avisa é a mensagem em português.
  return execFileSync("gh", argumentos, {
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

const executadoDiretamente =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (executadoDiretamente) {
  const tag = process.argv[2];
  if (!tag || !FORMATO_DA_TAG.test(tag)) {
    console.error(
      "Erro: informe a tag da release no formato vMAJOR.MINOR.PATCH. Uso: node scripts/conferir-release.mjs v0.1.0",
    );
    process.exit(1);
  }

  const pasta = mkdtempSync(join(tmpdir(), "memoteca-release-"));
  try {
    let release;
    try {
      release = JSON.parse(
        gh(["release", "view", tag, "--json", "assets,body"]),
      );
    } catch {
      console.error(
        `Erro: não consegui ler a release ${tag}. Confira se o gh está instalado e autenticado e se a release existe (rascunho conta).`,
      );
      process.exit(1);
    }
    const arquivos = release.assets.map((anexo) => anexo.name);

    let shaDoArquivo = "";
    if (arquivos.some((nome) => nome.endsWith(".exe"))) {
      gh(["release", "download", tag, "--pattern", "*.exe", "--dir", pasta]);
      const baixado = readdirSync(pasta).find((nome) => nome.endsWith(".exe"));
      shaDoArquivo = createHash("sha256")
        .update(readFileSync(join(pasta, baixado)))
        .digest("hex");
    }

    const problemas = problemasDaRelease({
      tag,
      arquivos,
      corpo: release.body ?? "",
      shaDoArquivo,
    });
    if (problemas.length > 0) {
      for (const problema of problemas) {
        console.error(`Erro: ${problema}`);
      }
      process.exit(1);
    }
    console.log(
      `Release ${tag} conferida: ${arquivos[0]}, SHA-256 ${shaDoArquivo}.`,
    );
  } finally {
    rmSync(pasta, { recursive: true, force: true });
  }
}
