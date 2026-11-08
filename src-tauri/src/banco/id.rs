//! Geração de IDs estáveis.

use uuid::Uuid;

/// Novo ID: UUIDv7 em texto. É único, ordenado por data de criação e nunca é reaproveitado.
pub fn novo_id() -> String {
    Uuid::now_v7().to_string()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_sao_uuid_v7_e_unicos() {
        let ids: HashSet<String> = (0..5_000).map(|_| novo_id()).collect();

        assert_eq!(ids.len(), 5_000);
        for id in ids.iter().take(10) {
            let uuid = Uuid::parse_str(id).expect("o ID deve ser um UUID válido");
            assert_eq!(uuid.get_version_num(), 7);
        }
    }

    #[test]
    fn ids_gerados_em_sequencia_mantem_a_ordem() {
        let primeiro = novo_id();
        let segundo = novo_id();

        assert!(primeiro < segundo);
    }
}
