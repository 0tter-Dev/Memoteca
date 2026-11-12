// Confere a tag de uma release antes de gerar qualquer coisa (E16).
//
// Uso: node scripts/validar-tag.mjs v0.1.0
//
// A tag precisa ter o formato `vMAJOR.MINOR.PATCH` (só números, sem zeros à esquerda) e a versão
// dela precisa ser a mesma do `package.json`, do `Cargo.toml`, do `Cargo.lock` e do
// `tauri.conf.json`. Assim o instalador, a janela e a release sempre dizem a mesma versão.
// Qualquer problema termina com código 1 e uma mensagem em português, e o workflow de release
// não gera nem publica nada.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const NUMERO = "(0|[1-9][0-9]*)";
const FORMATO_DA_TAG = new RegExp(`^v(${NUMERO}\\.${NUMERO}\\.${NUMERO})$`);

function ler(...caminho) {
  return readFileSync(join(RAIZ, ...caminho), "utf-8");
}

/** Versão do pacote `memoteca` no `Cargo.lock`. */
function versaoNoCargoLock(texto) {
  const bloco = texto
    .split(/\r?\n\r?\n/)
    .find((trecho) => /^name = "memoteca"$/m.test(trecho));
  return bloco?.match(/^version = "([^"]+)"$/m)?.[1] ?? null;
}

/** Versão do `[package]` no `Cargo.toml`. */
function versaoNoCargoToml(texto) {
  const pacote = texto
    .split(/^\[/m)
    .find((trecho) => trecho.startsWith("package]"));
  return pacote?.match(/^version = "([^"]+)"$/m)?.[1] ?? null;
}

function versoesDoProjeto() {
  return {
    "package.json": JSON.parse(ler("package.json")).version,
    "src-tauri/Cargo.toml": versaoNoCargoToml(ler("src-tauri", "Cargo.toml")),
    "src-tauri/Cargo.lock": versaoNoCargoLock(ler("src-tauri", "Cargo.lock")),
    "src-tauri/tauri.conf.json": JSON.parse(ler("src-tauri", "tauri.conf.json"))
      .version,
  };
}

/** Devolve a lista de problemas da tag, vazia se ela está certa. */
export function problemasDaTag(tag, versoes) {
  const formato = FORMATO_DA_TAG.exec(tag ?? "");
  if (!formato) {
    return [
      `A tag "${tag ?? ""}" não tem o formato esperado. Use vMAJOR.MINOR.PATCH, por exemplo v0.1.0.`,
    ];
  }
  const versao = formato[1];
  return Object.entries(versoes)
    .filter(([, valor]) => valor !== versao)
    .map(
      ([arquivo, valor]) =>
        `A tag é ${tag}, mas ${arquivo} está na versão ${valor ?? "(não encontrada)"}. Ajuste a versão para ${versao} antes de criar a tag.`,
    );
}

const executadoDiretamente =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (executadoDiretamente) {
  const tag = process.argv[2];
  const problemas = problemasDaTag(tag, versoesDoProjeto());
  if (problemas.length > 0) {
    for (const problema of problemas) {
      console.error(`Erro: ${problema}`);
    }
    process.exit(1);
  }
  console.log(`Tag ${tag} válida.`);
}
