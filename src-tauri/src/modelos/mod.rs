//! Escolha e download de modelos (E08): catálogo curado, validação do nome, regra de espaço
//! em disco e o download guiado, com progresso, cancelamento e mensagens amigáveis.
//!
//! **Download interrompido:** o Ollama guarda o que já baixou e retoma do ponto onde parou na
//! próxima chamada, e o modelo só aparece na lista quando está completo. O Memoteca não
//! apaga nada na pasta do Ollama: a recuperação é "Continuar o download".

pub mod catalogo;
pub mod espaco;
pub mod nome;
pub mod servico;

pub use catalogo::{Catalogo, ErroCatalogo, ModeloDoCatalogo, catalogo_embutido};
pub use espaco::{AvaliacaoDeEspaco, AvaliacaoParaInterface};
pub use nome::{ErroNome, NomeDeModelo};
pub use servico::{
    ErroModelo, FaseDoDownload, ProgressoDoModelo, ResultadoDoModelo, ServicoDeModelos,
};
