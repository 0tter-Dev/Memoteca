import { describe, expect, it } from "vitest";
import {
  MENSAGEM_PADRAO,
  mensagemDaFalha,
  mensagemDoErro,
  pareceTecnica,
} from "./erros";

/** Mensagens amigáveis de verdade, como o backend as devolve. Nenhuma pode ser barrada. */
const AMIGAVEIS = [
  "O Ollama não está respondendo. Verifique se ele está em execução e tente de novo.",
  "Outro programa está usando a porta do Ollama (11434). Feche esse programa e tente de novo.",
  "O Ollama instalado é a versão 0.29.9, e o Memoteca funciona melhor com a 0.30.0 ou mais recente. Atualize o Ollama para usar todos os recursos.",
  "Este modelo não está instalado. Baixe o modelo e tente de novo.",
  "Não encontramos esse modelo na biblioteca do Ollama. Confira o nome e tente de novo.",
  "A conexão com o Ollama caiu no meio da operação. Tente de novo.",
  "O Ollama enviou uma resposta inesperada. Tente de novo.",
  "Não foi possível abrir os dados do Memoteca. O arquivo pode estar em uso por outro programa, protegido ou danificado. Seus dados não foram apagados. Feche outros programas que possam estar usando o arquivo e abra o Memoteca de novo.",
  "Não há espaço livre no disco para salvar seus dados. Libere espaço e tente de novo.",
  "Estes dados foram criados por uma versão mais nova do Memoteca. Atualize o aplicativo para abri-los. Nada foi alterado.",
  "Não há espaço livre suficiente para baixar este modelo. São necessários 8,0 GB e há 1,0 GB livres. Libere espaço e tente de novo.",
  "O Windows exigiu permissão de administrador para instalar o Ollama, e o Memoteca não a solicita. Instale o Ollama pelo site oficial (ollama.com) e abra o Memoteca de novo.",
  "Este arquivo não é um backup do Memoteca, ou está incompleto ou danificado. Nada foi importado nem alterado. Escolha outro arquivo.",
  "Este backup usa o formato 7, que esta versão do Memoteca não conhece. Atualize o Memoteca e tente de novo. Nada foi importado nem alterado.",
  "O modelo não devolveu as sugestões num formato que o Memoteca entenda. Nenhuma memória foi criada. Tente de novo ou use outro modelo.",
  "O Memoteca não tem permissão para gravar nesse local. Escolha outra pasta, como Documentos. Nenhum arquivo foi criado.",
  "Uma memória pode ter no máximo 2000 caracteres. Resuma o texto.",
  "Escolha um modelo para esta conversa antes de enviar.",
  "O download foi interrompido, mas o que já foi baixado foi guardado. Clique em Continuar para retomar de onde parou.",
];

/** Como o texto cru de erros de verdade chega à tela se nada o filtrar. */
const TECNICAS = [
  "invalid args `conversaId` for command `enviar_mensagem`: command enviar_mensagem missing required key conversaId",
  "command listar_algo not found",
  "missing field `nome` at line 1 column 20",
  "unknown variant `xyz`, expected `global` or `projeto`",
  "TypeError: Cannot read properties of undefined (reading 'id')",
  "ReferenceError: x is not defined",
  "Error: connection refused",
  "Uncaught Exception in handler",
  "at carregar (http://localhost:1420/src/estado/projetos.ts:48:20)",
  "rusqlite::Error: SQLITE_BUSY: database is locked",
  "thread 'main' panicked at src/lib.rs:10:5",
  "HTTP 500 Internal Server Error",
  '{"error":"model \'x\' not found"}',
  '["a","b"]',
  "[object Object]",
  "undefined",
  "NaN",
  "C:\\Users\\gabri\\AppData\\Roaming\\memoteca\\memoteca.db",
  "\\\\?\\C:\\dados\\memoteca.db",
  "error sending request for url (http://127.0.0.1:11434/api/chat)",
  "connection refused (os error 10061)",
  "request timed out",
  "failed to open database",
  "Cannot read properties of null",
];

describe("pareceTecnica", () => {
  it("deixa passar todas as mensagens amigáveis do backend", () => {
    for (const mensagem of AMIGAVEIS) {
      expect(pareceTecnica(mensagem), mensagem).toBe(false);
    }
  });

  it("reconhece erros técnicos de Tauri, JavaScript, Rust, SQLite, HTTP e JSON", () => {
    for (const texto of TECNICAS) {
      expect(pareceTecnica(texto), texto).toBe(true);
    }
  });
});

describe("mensagemDoErro", () => {
  it("deixa passar a mensagem amigável como está", () => {
    for (const mensagem of AMIGAVEIS) {
      expect(mensagemDoErro(mensagem)).toBe(mensagem);
    }
  });

  it("troca texto técnico pela mensagem padrão, nunca mostra o erro cru", () => {
    for (const texto of TECNICAS) {
      expect(mensagemDoErro(texto), texto).toBe(MENSAGEM_PADRAO);
    }
  });

  it("o que não é texto (exceção, objeto, nulo) vira a mensagem padrão", () => {
    expect(mensagemDoErro(new TypeError("Cannot read properties"))).toBe(
      MENSAGEM_PADRAO,
    );
    expect(mensagemDoErro({ codigo: 42 })).toBe(MENSAGEM_PADRAO);
    expect(mensagemDoErro(null)).toBe(MENSAGEM_PADRAO);
    expect(mensagemDoErro(undefined)).toBe(MENSAGEM_PADRAO);
    expect(mensagemDoErro(42)).toBe(MENSAGEM_PADRAO);
  });

  it("texto vazio ou só com espaços vira a mensagem padrão", () => {
    expect(mensagemDoErro("")).toBe(MENSAGEM_PADRAO);
    expect(mensagemDoErro("   \n")).toBe(MENSAGEM_PADRAO);
  });

  it("usa o texto padrão que quem chama pediu", () => {
    expect(
      mensagemDoErro(new Error("x"), "Não foi possível baixar o modelo."),
    ).toBe("Não foi possível baixar o modelo.");
    expect(
      mensagemDoErro("TypeError: x", "Não foi possível baixar o modelo."),
    ).toBe("Não foi possível baixar o modelo.");
  });

  it("tira os espaços das pontas da mensagem amigável", () => {
    expect(mensagemDoErro("  Tente de novo.  ")).toBe("Tente de novo.");
  });
});

describe("mensagemDaFalha", () => {
  it("lê a mensagem de uma falha com objeto, e filtra o que for técnico", () => {
    expect(
      mensagemDaFalha({ mensagem: AMIGAVEIS[0], mensagemParcial: null }),
    ).toBe(AMIGAVEIS[0]);
    expect(
      mensagemDaFalha({ mensagem: "TypeError: x", podeContinuar: false }),
    ).toBe(MENSAGEM_PADRAO);
    expect(mensagemDaFalha({ mensagem: 42 })).toBe(MENSAGEM_PADRAO);
  });

  it("sem `mensagem`, trata como erro comum", () => {
    expect(mensagemDaFalha(AMIGAVEIS[1])).toBe(AMIGAVEIS[1]);
    expect(mensagemDaFalha({ outra: "coisa" })).toBe(MENSAGEM_PADRAO);
    expect(mensagemDaFalha(null, "Padrão próprio.")).toBe("Padrão próprio.");
  });
});
