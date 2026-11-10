//! Sugestão de memórias sob demanda (E12).
//!
//! O usuário aperta um botão, o app faz **uma chamada extra** ao modelo da própria conversa
//! (`POST /api/chat` sem streaming, com a resposta restrita a um JSON Schema) e devolve uma
//! lista de sugestões. Este módulo **nunca grava nada**: nem memória, nem mensagem. Só a
//! aprovação explícita de cada sugestão, feita pela interface, cria uma memória. Por isso a
//! chamada também não altera o histórico da conversa.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::banco::{Banco, ErroBanco, MEMORIA_MAXIMA, Mensagem, Papel};
use crate::ollama::{ClienteOllama, ErroOllama, MensagemPedido, PapelChat, PedidoEstruturado};

use super::geracao::{com_banco, keep_alive_do_ollama, num_ctx_efetivo};

/// Quantas sugestões, no máximo, vão para a tela.
pub const MAXIMO_DE_SUGESTOES: usize = 5;

/// Quanto da conversa, em caracteres, entra no pedido. Fica o trecho mais recente. Mantém a
/// chamada extra rápida e dentro da janela de contexto de modelos pequenos.
pub const CONVERSA_MAXIMA_NO_PEDIDO: usize = 12_000;

/// Temperatura baixa: a tarefa é extrair fatos, não criar.
const TEMPERATURA_DAS_SUGESTOES: f64 = 0.2;

const INSTRUCAO: &str = "\
Você ajuda a manter a memória de um assistente. Leia a conversa e extraia as memórias que \
valem a pena guardar para conversas futuras.

Uma memória é um fato curto e duradouro sobre o usuário, sobre as preferências dele ou sobre \
o que ele está construindo. Escreva cada uma como uma frase curta em português, na terceira \
pessoa (por exemplo: \"O usuário trabalha com Rust.\").

Não inclua assuntos passageiros, perguntas, respostas do assistente, nem informações que já \
estão nas memórias existentes. No máximo 5 memórias. Se nada merece ser guardado, devolva a \
lista vazia.

Responda só com JSON no formato pedido.";

const PEDIDO_FINAL: &str = "Sugira as memórias que valem a pena guardar desta conversa.";

/// Uma sugestão de memória, ainda sem escopo: o usuário escolhe ao aprovar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SugestaoDeMemoria {
    pub texto: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoDaSugestao {
    Sugestoes(Vec<SugestaoDeMemoria>),
    /// O usuário cancelou. Nada foi criado.
    Cancelada,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErroSugestao {
    /// A conversa ainda não tem um modelo escolhido.
    SemModelo,
    /// A conversa não tem mensagens para analisar.
    ConversaVazia,
    /// O modelo respondeu algo fora do esquema, ou nada.
    RespostaInvalida,
    Banco(ErroBanco),
    Ollama(ErroOllama),
}

impl ErroSugestao {
    pub fn mensagem_amigavel(&self) -> String {
        match self {
            Self::SemModelo => {
                "Escolha um modelo para esta conversa antes de pedir sugestões.".to_owned()
            }
            Self::ConversaVazia => {
                "Converse um pouco antes de pedir sugestões de memória.".to_owned()
            }
            Self::RespostaInvalida => "O modelo não devolveu as sugestões num formato que o \
                Memoteca entenda. Nenhuma memória foi criada. Tente de novo ou use outro modelo."
                .to_owned(),
            Self::Banco(erro) => erro.mensagem_amigavel(),
            Self::Ollama(erro) => erro.mensagem_amigavel(),
        }
    }
}

impl std::fmt::Display for ErroSugestao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemModelo => write!(f, "a conversa não tem modelo"),
            Self::ConversaVazia => write!(f, "a conversa não tem mensagens"),
            Self::RespostaInvalida => write!(f, "resposta fora do esquema das sugestões"),
            Self::Banco(erro) => write!(f, "{erro}"),
            Self::Ollama(erro) => write!(f, "{erro}"),
        }
    }
}

impl std::error::Error for ErroSugestao {}

impl From<ErroBanco> for ErroSugestao {
    fn from(erro: ErroBanco) -> Self {
        Self::Banco(erro)
    }
}

/// O esquema da resposta: `{"sugestoes": [{"texto": "..."}]}`.
pub fn esquema_das_sugestoes() -> Value {
    json!({
        "type": "object",
        "properties": {
            "sugestoes": {
                "type": "array",
                "maxItems": MAXIMO_DE_SUGESTOES,
                "items": {
                    "type": "object",
                    "properties": { "texto": { "type": "string" } },
                    "required": ["texto"]
                }
            }
        },
        "required": ["sugestoes"]
    })
}

/// A conversa como texto corrido. Fica só o trecho mais recente que cabe em `limite`
/// caracteres, e avisa quando o começo foi deixado de fora.
pub fn transcrever(mensagens: &[Mensagem], limite: usize) -> String {
    let mut blocos: Vec<String> = Vec::new();
    let mut tamanho = 0;
    let mut cortou = false;
    for mensagem in mensagens.iter().rev() {
        let quem = match mensagem.papel {
            Papel::Usuario => "Usuário",
            Papel::Assistente => "Assistente",
        };
        let bloco = format!("{quem}: {}", mensagem.conteudo.trim());
        let custo = bloco.chars().count() + 2;
        if !blocos.is_empty() && tamanho + custo > limite {
            cortou = true;
            break;
        }
        tamanho += custo;
        blocos.push(bloco);
    }
    blocos.reverse();
    let mut texto = String::new();
    if cortou {
        texto.push_str("(o começo da conversa foi omitido)\n\n");
    }
    texto.push_str(&blocos.join("\n\n"));
    texto
}

/// As mensagens do pedido: a instrução (com as memórias já guardadas, para não repetir) e a
/// conversa seguida do pedido final.
pub fn montar_mensagens(existentes: &[String], mensagens: &[Mensagem]) -> Vec<MensagemPedido> {
    let mut sistema = INSTRUCAO.to_owned();
    if !existentes.is_empty() {
        sistema.push_str("\n\nMemórias já guardadas (não repita):");
        for memoria in existentes {
            sistema.push_str("\n- ");
            sistema.push_str(&memoria.trim().replace('\n', "\n  "));
        }
    }
    let conversa = transcrever(mensagens, CONVERSA_MAXIMA_NO_PEDIDO);
    vec![
        MensagemPedido {
            papel: PapelChat::Sistema,
            conteudo: sistema,
        },
        MensagemPedido {
            papel: PapelChat::Usuario,
            conteudo: format!("Conversa:\n\n{conversa}\n\n---\n{PEDIDO_FINAL}"),
        },
    ]
}

#[derive(Deserialize)]
struct RespostaBruta {
    sugestoes: Vec<ItemBruto>,
}

#[derive(Deserialize)]
struct ItemBruto {
    texto: String,
}

/// Lê e limpa a resposta do modelo. Fora do esquema, ou vazia, é `RespostaInvalida`. Uma lista
/// vazia **não é erro**: significa que nada merece ser guardado.
///
/// Fica de fora: texto em branco, texto maior que o limite de uma memória, repetidas (entre
/// si ou com as que já existem) e o que passar de `MAXIMO_DE_SUGESTOES`.
pub fn interpretar_resposta(
    conteudo: &str,
    existentes: &[String],
) -> Result<Vec<SugestaoDeMemoria>, ErroSugestao> {
    let conteudo = conteudo.trim();
    if conteudo.is_empty() {
        return Err(ErroSugestao::RespostaInvalida);
    }
    let bruta: RespostaBruta =
        serde_json::from_str(conteudo).map_err(|_| ErroSugestao::RespostaInvalida)?;

    let chave = |texto: &str| texto.trim().to_lowercase();
    let mut vistas: Vec<String> = existentes.iter().map(|texto| chave(texto)).collect();
    let mut sugestoes = Vec::new();
    for item in bruta.sugestoes {
        let texto = item.texto.trim();
        if texto.is_empty() || texto.chars().count() > MEMORIA_MAXIMA {
            continue;
        }
        let ja_vista = chave(texto);
        if vistas.contains(&ja_vista) {
            continue;
        }
        vistas.push(ja_vista);
        sugestoes.push(SugestaoDeMemoria {
            texto: texto.to_owned(),
        });
        if sugestoes.len() == MAXIMO_DE_SUGESTOES {
            break;
        }
    }
    Ok(sugestoes)
}

/// Pede ao modelo da conversa as sugestões de memória. Não grava nada.
pub async fn sugerir(
    banco: &Mutex<Banco>,
    cliente: &ClienteOllama,
    conversa_id: &str,
    cancelamento: &CancellationToken,
) -> Result<DesfechoDaSugestao, ErroSugestao> {
    let (conversa, mensagens, existentes) = com_banco(banco, |b| {
        let conversa = b.obter_conversa(conversa_id)?;
        let mensagens = b.listar_mensagens(conversa_id)?;
        let mut existentes: Vec<String> = b
            .listar_memorias_globais()?
            .into_iter()
            .map(|m| m.conteudo)
            .collect();
        existentes.extend(
            b.listar_memorias_do_projeto(&conversa.projeto_id)?
                .into_iter()
                .map(|m| m.conteudo),
        );
        Ok((conversa, mensagens, existentes))
    })?;

    let modelo = conversa
        .modelo
        .clone()
        .filter(|modelo| !modelo.trim().is_empty())
        .ok_or(ErroSugestao::SemModelo)?;
    if mensagens.is_empty() {
        return Err(ErroSugestao::ConversaVazia);
    }

    // A janela de contexto limita o `num_ctx`, como no chat. Se não der para consultar, segue
    // sem o limite, mas modelo ausente ou Ollama fora do ar são erros de verdade.
    let janela = match cliente.detalhes_do_modelo(&modelo).await {
        Ok(detalhes) => {
            detalhes.exige_conversa().map_err(ErroSugestao::Ollama)?;
            detalhes.janela_de_contexto
        }
        Err(
            erro @ (ErroOllama::ModeloNaoEncontrado
            | ErroOllama::OllamaForaDoAr
            | ErroOllama::PortaOcupada),
        ) => return Err(ErroSugestao::Ollama(erro)),
        Err(_) => None,
    };

    let pedido = PedidoEstruturado {
        modelo,
        mensagens: montar_mensagens(&existentes, &mensagens),
        esquema: esquema_das_sugestoes(),
        temperature: Some(TEMPERATURA_DAS_SUGESTOES),
        num_ctx: num_ctx_efetivo(conversa.num_ctx, janela),
        keep_alive: conversa.keep_alive.as_deref().map(keep_alive_do_ollama),
    };

    match cliente
        .conversar_estruturado(&pedido, cancelamento)
        .await
        .map_err(ErroSugestao::Ollama)?
    {
        None => Ok(DesfechoDaSugestao::Cancelada),
        Some(conteudo) => {
            interpretar_resposta(&conteudo, &existentes).map(DesfechoDaSugestao::Sugestoes)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::EstadoMensagem;

    fn mensagem(papel: Papel, conteudo: &str) -> Mensagem {
        Mensagem {
            id: "i".into(),
            conversa_id: "c".into(),
            posicao: 1,
            papel,
            conteudo: conteudo.into(),
            raciocinio: Some("segredo do raciocínio".into()),
            estado: EstadoMensagem::Completa,
            criado_em: String::new(),
            atualizado_em: String::new(),
        }
    }

    fn textos(sugestoes: Vec<SugestaoDeMemoria>) -> Vec<String> {
        sugestoes.into_iter().map(|s| s.texto).collect()
    }

    #[test]
    fn a_conversa_vira_texto_corrido_sem_o_raciocinio() {
        let texto = transcrever(
            &[
                mensagem(Papel::Usuario, "Oi, sou a Marina."),
                mensagem(Papel::Assistente, "Olá, Marina!"),
            ],
            1_000,
        );

        assert_eq!(
            texto,
            "Usuário: Oi, sou a Marina.\n\nAssistente: Olá, Marina!"
        );
        assert!(!texto.contains("segredo"));
    }

    #[test]
    fn conversa_longa_fica_so_com_o_trecho_mais_recente_e_avisa() {
        let mensagens = [
            mensagem(Papel::Usuario, &"a".repeat(50)),
            mensagem(Papel::Assistente, &"b".repeat(50)),
            mensagem(Papel::Usuario, &"c".repeat(50)),
        ];

        let texto = transcrever(&mensagens, 130);

        assert!(texto.starts_with("(o começo da conversa foi omitido)"));
        assert!(!texto.contains('a'.to_string().repeat(50).as_str()));
        assert!(texto.contains(&"c".repeat(50)));
        assert!(texto.contains(&"b".repeat(50)));
    }

    #[test]
    fn a_ultima_mensagem_entra_mesmo_que_sozinha_passe_do_limite() {
        let texto = transcrever(&[mensagem(Papel::Usuario, &"x".repeat(500))], 100);

        assert!(texto.contains(&"x".repeat(500)));
        assert!(!texto.contains("omitido"));
    }

    #[test]
    fn o_pedido_traz_a_instrucao_a_conversa_e_as_memorias_existentes_para_nao_repetir() {
        let mensagens = montar_mensagens(
            &["Mora em Recife".to_owned()],
            &[mensagem(Papel::Usuario, "Trabalho com Rust.")],
        );

        assert_eq!(mensagens.len(), 2);
        assert_eq!(mensagens[0].papel, PapelChat::Sistema);
        assert!(
            mensagens[0]
                .conteudo
                .contains("Memórias já guardadas (não repita):\n- Mora em Recife")
        );
        assert_eq!(mensagens[1].papel, PapelChat::Usuario);
        assert!(
            mensagens[1]
                .conteudo
                .contains("Usuário: Trabalho com Rust.")
        );
        assert!(mensagens[1].conteudo.ends_with(PEDIDO_FINAL));

        let sem_existentes = montar_mensagens(&[], &[mensagem(Papel::Usuario, "Oi")]);
        assert!(!sem_existentes[0].conteudo.contains("já guardadas"));
    }

    #[test]
    fn resposta_valida_vira_sugestoes_limpas() {
        let resposta = r#"{"sugestoes":[{"texto":"  O usuário trabalha com Rust. "},{"texto":"Prefere respostas curtas."}]}"#;

        let sugestoes = interpretar_resposta(resposta, &[]).unwrap();

        assert_eq!(
            textos(sugestoes),
            ["O usuário trabalha com Rust.", "Prefere respostas curtas."]
        );
    }

    #[test]
    fn lista_vazia_nao_e_erro() {
        assert_eq!(
            interpretar_resposta(r#"{"sugestoes":[]}"#, &[]).unwrap(),
            []
        );
    }

    #[test]
    fn resposta_fora_do_esquema_ou_vazia_e_invalida() {
        for resposta in [
            "",
            "   \n",
            "isto não é JSON",
            "{}",
            "[]",
            r#"{"sugestoes":"nada"}"#,
            r#"{"sugestoes":[{"texto":42}]}"#,
            r#"{"sugestoes":[{"outro":"x"}]}"#,
            r#"{"sugestoes":["solta"]}"#,
            r#"{"sugestoes":[{"texto":"cortado"#,
        ] {
            assert_eq!(
                interpretar_resposta(resposta, &[]),
                Err(ErroSugestao::RespostaInvalida),
                "{resposta:?}"
            );
        }
    }

    #[test]
    fn descarta_em_branco_grandes_demais_e_repetidas() {
        let grande = "g".repeat(MEMORIA_MAXIMA + 1);
        let resposta = json!({ "sugestoes": [
            { "texto": "   " },
            { "texto": grande },
            { "texto": "Gosta de café" },
            { "texto": "  gosta de CAFÉ " },
            { "texto": "Já existe" },
            { "texto": "Outra" },
        ]})
        .to_string();

        let sugestoes = interpretar_resposta(&resposta, &["já existe".to_owned()]).unwrap();

        assert_eq!(textos(sugestoes), ["Gosta de café", "Outra"]);
    }

    #[test]
    fn nunca_passa_do_maximo_de_sugestoes() {
        let itens: Vec<Value> = (0..9)
            .map(|i| json!({ "texto": format!("Fato {i}") }))
            .collect();
        let resposta = json!({ "sugestoes": itens }).to_string();

        assert_eq!(
            interpretar_resposta(&resposta, &[]).unwrap().len(),
            MAXIMO_DE_SUGESTOES
        );
    }

    #[test]
    fn o_esquema_pede_a_lista_de_objetos_com_texto() {
        let esquema = esquema_das_sugestoes();

        assert_eq!(esquema["required"], json!(["sugestoes"]));
        assert_eq!(
            esquema["properties"]["sugestoes"]["items"]["required"],
            json!(["texto"])
        );
        assert_eq!(esquema["properties"]["sugestoes"]["maxItems"], 5);
    }
}
