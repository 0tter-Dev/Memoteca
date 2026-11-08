//! Divisão de um stream de bytes em linhas NDJSON.
//!
//! Os pedaços que chegam da rede podem cortar uma linha (ou até um caractere UTF-8) no
//! meio. Por isso o divisor guarda **bytes** e só entrega uma linha quando ela está
//! completa. A conversão para texto fica a cargo de quem interpreta a linha.

/// Tamanho máximo aceito para uma única linha. Acima disso o stream é considerado inválido.
pub const TAMANHO_MAXIMO_DA_LINHA: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct LinhaGrandeDemais;

#[derive(Default)]
pub struct DivisorDeLinhas {
    pendente: Vec<u8>,
}

impl DivisorDeLinhas {
    pub fn novo() -> Self {
        Self::default()
    }

    /// Acrescenta bytes e devolve as linhas que ficaram completas, sem o `\n` (e sem o
    /// `\r` anterior, se houver). Linhas vazias são descartadas.
    pub fn empurrar(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, LinhaGrandeDemais> {
        self.pendente.extend_from_slice(bytes);

        let mut linhas = Vec::new();
        let mut inicio = 0;
        while let Some(posicao) = self.pendente[inicio..].iter().position(|&b| b == b'\n') {
            let fim = inicio + posicao;
            if let Some(linha) = limpar(&self.pendente[inicio..fim]) {
                linhas.push(linha);
            }
            inicio = fim + 1;
        }
        self.pendente.drain(..inicio);

        if self.pendente.len() > TAMANHO_MAXIMO_DA_LINHA {
            return Err(LinhaGrandeDemais);
        }
        Ok(linhas)
    }

    /// Fim do stream: devolve o que sobrou, se for uma linha sem `\n` final.
    pub fn finalizar(&mut self) -> Option<Vec<u8>> {
        let resto = limpar(&self.pendente);
        self.pendente.clear();
        resto
    }
}

fn limpar(linha: &[u8]) -> Option<Vec<u8>> {
    let linha = linha.strip_suffix(b"\r").unwrap_or(linha);
    if linha.iter().all(u8::is_ascii_whitespace) {
        None
    } else {
        Some(linha.to_vec())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn texto(linhas: Vec<Vec<u8>>) -> Vec<String> {
        linhas
            .into_iter()
            .map(|l| String::from_utf8(l).unwrap())
            .collect()
    }

    #[test]
    fn separa_varias_linhas_de_um_mesmo_pedaco() {
        let mut divisor = DivisorDeLinhas::novo();

        let linhas = divisor.empurrar(b"{\"a\":1}\n{\"b\":2}\n").unwrap();

        assert_eq!(texto(linhas), ["{\"a\":1}", "{\"b\":2}"]);
        assert_eq!(divisor.finalizar(), None);
    }

    #[test]
    fn reconstroi_uma_linha_cortada_em_qualquer_ponto() {
        let original = "{\"message\":{\"content\":\"olá, mundo\"}}\n{\"done\":true}\n";
        let bytes = original.as_bytes();

        // Corta o stream em todas as posições possíveis, inclusive no meio do `á` (UTF-8).
        for corte in 0..=bytes.len() {
            let mut divisor = DivisorDeLinhas::novo();
            let mut linhas = divisor.empurrar(&bytes[..corte]).unwrap();
            linhas.extend(divisor.empurrar(&bytes[corte..]).unwrap());

            assert_eq!(
                texto(linhas),
                [
                    "{\"message\":{\"content\":\"olá, mundo\"}}",
                    "{\"done\":true}"
                ],
                "corte em {corte}"
            );
        }
    }

    #[test]
    fn funciona_com_um_byte_por_vez() {
        let mut divisor = DivisorDeLinhas::novo();
        let mut linhas = Vec::new();
        for byte in "{\"x\":\"ç\"}\n{\"y\":2}\n".as_bytes() {
            linhas.extend(divisor.empurrar(&[*byte]).unwrap());
        }

        assert_eq!(texto(linhas), ["{\"x\":\"ç\"}", "{\"y\":2}"]);
    }

    #[test]
    fn aceita_fim_de_linha_do_windows_e_descarta_linhas_vazias() {
        let mut divisor = DivisorDeLinhas::novo();

        let linhas = divisor
            .empurrar(b"{\"a\":1}\r\n\r\n\n{\"b\":2}\r\n")
            .unwrap();

        assert_eq!(texto(linhas), ["{\"a\":1}", "{\"b\":2}"]);
    }

    #[test]
    fn a_ultima_linha_sem_quebra_e_entregue_no_fim() {
        let mut divisor = DivisorDeLinhas::novo();

        assert!(divisor.empurrar(b"{\"done\":true}").unwrap().is_empty());

        assert_eq!(divisor.finalizar().unwrap(), b"{\"done\":true}");
        assert_eq!(divisor.finalizar(), None);
    }

    #[test]
    fn linha_enorme_sem_quebra_e_recusada() {
        let mut divisor = DivisorDeLinhas::novo();
        let bloco = vec![b'x'; 1024 * 1024];

        let mut recusou = false;
        for _ in 0..9 {
            if divisor.empurrar(&bloco).is_err() {
                recusou = true;
                break;
            }
        }
        assert!(recusou);
    }
}
