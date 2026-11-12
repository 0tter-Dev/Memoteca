import { defineConfig } from "@playwright/test";

// Testes ponta a ponta (E15): o Memoteca de verdade (build com o recurso `e2e`), com a janela
// controlada por CDP e um servidor Ollama falso. Veja `e2e/apoio`.
export default defineConfig({
  testDir: "e2e",
  testMatch: "**/*.e2e.ts",
  outputDir: "e2e/resultados",
  globalSetup: "./e2e/apoio/preparo.ts",
  // Um app por vez: todos usam a porta 11434 do Ollama falso.
  fullyParallel: false,
  workers: 1,
  retries: 0,
  // No CI, para no primeiro erro: um app que não abre derrubaria os 17 testes, um por vez.
  maxFailures: process.env.CI ? 1 : 0,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  use: { actionTimeout: 15_000 },
  reporter: [["list"]],
});
