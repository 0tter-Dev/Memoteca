// Confere o instalador e o executável gerados (E16), antes de qualquer publicação.
//
// Uso: node scripts/verificar-pacote.mjs <instalador.exe> <memoteca.exe>
//
// Falha (código 1, mensagem em português) se:
//  - o nome do produto nos metadados do instalador ou do executável não é "Memoteca";
//  - o texto "OllamaUI" (o nome provisório do projeto) aparece no instalador ou no executável;
//  - o executável traz o recurso `e2e` dos testes, que nunca pode ir para um release;
//  - o nome do arquivo do instalador não começa com "Memoteca".
// Lê os metadados com o PowerShell do Windows, sem dependência nova.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** Textos que nunca podem estar no pacote, como UTF-8 e como UTF-16 (strings do Windows). */
const PROIBIDOS = [
  { texto: "OllamaUI", motivo: 'o nome provisório "OllamaUI"' },
  {
    texto: "MEMOTECA_E2E_DADOS",
    motivo: "o recurso de testes `e2e` (nunca pode entrar no release)",
    soNoExecutavel: true,
  },
];

function contem(conteudo, texto) {
  return (
    conteudo.includes(Buffer.from(texto, "utf8")) ||
    conteudo.includes(Buffer.from(texto, "utf16le"))
  );
}

/** Metadados de versão do arquivo, lidos pelo PowerShell. */
function metadados(caminho) {
  const script =
    "$v = (Get-Item -LiteralPath $env:MEMOTECA_ARQUIVO).VersionInfo; " +
    "@{ ProductName = $v.ProductName; FileDescription = $v.FileDescription; " +
    "CompanyName = $v.CompanyName; ProductVersion = $v.ProductVersion } | ConvertTo-Json";
  const saida = execFileSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    {
      encoding: "utf-8",
      env: { ...process.env, MEMOTECA_ARQUIVO: caminho },
    },
  );
  return JSON.parse(saida);
}

/** Lista de problemas encontrados no instalador e no executável. */
export function problemasDoPacote(instalador, executavel) {
  const problemas = [];

  if (!basename(instalador).startsWith("Memoteca")) {
    problemas.push(
      `O instalador se chama "${basename(instalador)}", e o nome precisa começar com Memoteca.`,
    );
  }

  for (const [rotulo, caminho] of [
    ["instalador", instalador],
    ["executável", executavel],
  ]) {
    const dados = metadados(caminho);
    if (dados.ProductName !== "Memoteca") {
      problemas.push(
        `O nome do produto no ${rotulo} é "${dados.ProductName ?? ""}", e deveria ser "Memoteca".`,
      );
    }
    const conteudo = readFileSync(caminho);
    for (const proibido of PROIBIDOS) {
      if (proibido.soNoExecutavel && rotulo !== "executável") continue;
      if (contem(conteudo, proibido.texto)) {
        problemas.push(`O ${rotulo} contém ${proibido.motivo}.`);
      }
    }
  }
  return problemas;
}

const executadoDiretamente =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (executadoDiretamente) {
  const [instalador, executavel] = process.argv.slice(2);
  if (!instalador || !executavel) {
    console.error(
      "Uso: node scripts/verificar-pacote.mjs <instalador.exe> <memoteca.exe>",
    );
    process.exit(1);
  }
  const problemas = problemasDoPacote(instalador, executavel);
  if (problemas.length > 0) {
    for (const problema of problemas) {
      console.error(`Erro: ${problema}`);
    }
    process.exit(1);
  }
  console.log(`Pacote conferido: ${basename(instalador)}.`);
}
