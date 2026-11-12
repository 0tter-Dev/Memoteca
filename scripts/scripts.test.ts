import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { describe, expect, it } from "vitest";

const RAIZ = resolve(import.meta.dirname, "..");
const VALIDAR_TAG = join(RAIZ, "scripts", "validar-tag.mjs");
const VERIFICAR_PACOTE = join(RAIZ, "scripts", "verificar-pacote.mjs");

function validar(tag?: string) {
  const argumentos = tag === undefined ? [VALIDAR_TAG] : [VALIDAR_TAG, tag];
  return spawnSync(process.execPath, argumentos, { encoding: "utf-8" });
}

const versaoAtual = (
  JSON.parse(readFileSync(join(RAIZ, "package.json"), "utf-8")) as {
    version: string;
  }
).version;

describe("validar-tag (E16)", () => {
  it("aceita a tag da versão atual do projeto", () => {
    const resultado = validar(`v${versaoAtual}`);
    expect(resultado.status).toBe(0);
    expect(resultado.stdout).toContain("válida");
  });

  it.each([
    "0.1.0",
    "v0.1",
    "v0.1.0.1",
    "v01.0.0",
    "v0.1.0-rc.1",
    "v0.1.0+build",
    "versao1",
    "v",
    "V0.1.0",
    " v0.1.0",
    "",
  ])("rejeita a tag com formato inválido %j", (tag) => {
    const resultado = validar(tag);
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain("formato esperado");
    expect(resultado.stderr).not.toMatch(/at |node:internal|TypeError/);
  });

  it("rejeita quando falta a tag", () => {
    const resultado = validar();
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain("formato esperado");
  });

  it("rejeita uma tag bem formada com versão diferente da do código", () => {
    const resultado = validar("v9.9.9");
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain("package.json");
    expect(resultado.stderr).toContain("tauri.conf.json");
    expect(resultado.stderr).toContain("Cargo.toml");
    expect(resultado.stderr).toContain("Cargo.lock");
  });
});

describe("verificar-pacote (E16)", () => {
  it("falha, com mensagem clara, quando o produto não se chama Memoteca", () => {
    // O `node.exe` é um executável de verdade, mas o produto dele é "Node.js".
    const resultado = spawnSync(
      process.execPath,
      [VERIFICAR_PACOTE, process.execPath, process.execPath],
      { encoding: "utf-8" },
    );
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain('deveria ser "Memoteca"');
  });

  it("pede os dois caminhos quando faltam argumentos", () => {
    const resultado = spawnSync(process.execPath, [VERIFICAR_PACOTE], {
      encoding: "utf-8",
    });
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain("Uso:");
  });
});
