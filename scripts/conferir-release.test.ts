import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { describe, expect, it } from "vitest";
import {
  afirmacoesDeAssinatura,
  avisaQueNaoTemAssinatura,
  problemasDaRelease,
  shaDasNotas,
} from "./conferir-release.mjs";

const RAIZ = resolve(import.meta.dirname, "..");
const SHA = "a".repeat(64);

function arquivo(...caminho: string[]): string {
  return readFileSync(join(RAIZ, ...caminho), "utf-8");
}

/** Notas como o workflow as monta: o texto fixo mais a linha do SHA-256. */
function notas(sha: string = SHA): string {
  return `${arquivo(".github", "notas-da-release.md")}\n**SHA-256 do instalador:** \`${sha}\`\n`;
}

const certa = {
  tag: "v0.1.0",
  arquivos: ["Memoteca_0.1.0_x64-setup.exe"],
  corpo: notas(),
  shaDoArquivo: SHA,
};

describe("conferir-release (E17)", () => {
  it("aceita uma release como o workflow a monta", () => {
    expect(problemasDaRelease(certa)).toEqual([]);
  });

  it("lê o SHA-256 das notas, em minúsculas", () => {
    expect(shaDasNotas(notas("AB".repeat(32)))).toBe("ab".repeat(32));
    expect(shaDasNotas("sem soma nenhuma")).toBeNull();
  });

  it.each(["0.1.0", "v0.1", "v0.1.0-rc.1", ""])(
    "recusa a tag com formato inválido %j",
    (tag) => {
      const problemas = problemasDaRelease({ ...certa, tag });
      expect(problemas).toHaveLength(1);
      expect(problemas[0]).toContain("formato esperado");
    },
  );

  it("recusa quando o SHA-256 das notas não é o do arquivo", () => {
    const problemas = problemasDaRelease({
      ...certa,
      shaDoArquivo: "b".repeat(64),
    });
    expect(problemas.join(" ")).toContain("não é o do arquivo baixado");
  });

  it("recusa notas sem SHA-256", () => {
    const problemas = problemasDaRelease({
      ...certa,
      corpo: arquivo(".github", "notas-da-release.md"),
    });
    expect(problemas.join(" ")).toContain("não trazem o SHA-256");
  });

  it.each([
    [[], "nenhum anexo"],
    [["Memoteca_0.2.0_x64-setup.exe"], "Memoteca_0.1.0_x64-setup.exe"],
    [
      ["Memoteca_0.1.0_x64-setup.exe", "Memoteca_0.1.0_x64-setup (1).exe"],
      "só o instalador",
    ],
  ])("recusa anexos errados %j", (arquivos, trecho) => {
    const problemas = problemasDaRelease({ ...certa, arquivos });
    expect(problemas.join(" ")).toContain(trecho);
  });

  it("recusa notas que afirmam que o instalador é assinado e notas que esquecem o aviso", () => {
    const afirma = problemasDaRelease({
      ...certa,
      corpo: `${notas()}\nO instalador é assinado digitalmente.\n`,
    });
    expect(afirma.join(" ")).toContain("afirmam que o instalador é assinado");

    const esquece = problemasDaRelease({
      ...certa,
      corpo: `Memoteca\n**SHA-256 do instalador:** \`${SHA}\`\n`,
    });
    expect(esquece.join(" ")).toContain("ainda não tem assinatura de código");
  });

  it("frases que negam a assinatura não contam como afirmação", () => {
    expect(
      afirmacoesDeAssinatura("O instalador ainda não é assinado."),
    ).toEqual([]);
    expect(afirmacoesDeAssinatura("O instalador é assinado.")).toHaveLength(1);
    expect(afirmacoesDeAssinatura("Publicador verificado: Memoteca")).toEqual([
      "Publicador verificado: Memoteca",
    ]);
  });

  it("falha com mensagem amigável quando a tag não vem", () => {
    const resultado = spawnSync(
      process.execPath,
      [join(RAIZ, "scripts", "conferir-release.mjs")],
      { encoding: "utf-8" },
    );
    expect(resultado.status).toBe(1);
    expect(resultado.stderr).toContain("informe a tag");
    expect(resultado.stderr).not.toMatch(/node:internal|TypeError/);
  });
});

/**
 * Critério da E17 (falha): nenhum texto da release, do README ou do app afirma que o
 * instalador é assinado, e os textos que o usuário lê avisam que não é.
 */
describe("textos sobre a assinatura (E17)", () => {
  it.each([
    ["README.md", arquivo("README.md")],
    ["notas da release", arquivo(".github", "notas-da-release.md")],
    ["tela Sobre", arquivo("src", "TelaSobre.tsx")],
    ["aviso de independência", arquivo("src", "avisoDeIndependencia.ts")],
    [
      "página de licença do instalador",
      arquivo("src-tauri", "instalador", "licenca.txt"),
    ],
    ["config do Tauri", arquivo("src-tauri", "tauri.conf.json")],
    ["workflow de release", arquivo(".github", "workflows", "release.yml")],
  ])("%s não afirma que o instalador é assinado", (_nome, texto) => {
    expect(afirmacoesDeAssinatura(texto)).toEqual([]);
  });

  it("o README e as notas avisam que o instalador ainda não tem assinatura", () => {
    expect(avisaQueNaoTemAssinatura(arquivo("README.md"))).toBe(true);
    expect(
      avisaQueNaoTemAssinatura(arquivo(".github", "notas-da-release.md")),
    ).toBe(true);
  });
});
