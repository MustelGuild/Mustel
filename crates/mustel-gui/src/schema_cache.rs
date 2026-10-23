//! Esquema carregado na janela: cache por `(servidor, banco)` e o estado do carregamento.
//!
//! O carregamento em si é do núcleo ([`mustel_core::usecases::schema::load`]); aqui só se guarda o
//! resultado, para o autocomplete **não** falar com o banco a cada tecla.

use std::collections::BTreeMap;

use mustel_core::usecases::schema::Schema;

/// Chave do cache: o servidor e o banco de referência.
pub type SchemaKey = (String, String);

/// Esquemas carregados, por `(servidor, banco)`.
#[derive(Debug, Default)]
pub struct SchemaCache {
    loaded: BTreeMap<SchemaKey, Schema>,
}

impl SchemaCache {
    /// Esquema já carregado, se houver.
    pub fn get(&self, server: &str, database: &str) -> Option<&Schema> {
        self.loaded.get(&(server.to_string(), database.to_string()))
    }

    /// Guarda o resultado de um carregamento.
    pub fn insert(&mut self, server: &str, database: &str, schema: Schema) {
        self.loaded
            .insert((server.to_string(), database.to_string()), schema);
    }

    /// Esquece o esquema de um par — usado pelo `⟳` antes de recarregar.
    pub fn invalidate(&mut self, server: &str, database: &str) {
        self.loaded
            .remove(&(server.to_string(), database.to_string()));
    }

    /// Esquece **todos** os esquemas de um servidor.
    ///
    /// É o que resolve o caso de o servidor sair do cadastro: sem isto, recadastrar um servidor com o
    /// mesmo nome e outro banco traria de volta o esquema antigo, como se estivesse carregado.
    pub fn forget_server(&mut self, server: &str) {
        self.loaded.retain(|(name, _), _| name != server);
    }

    /// Quantos esquemas estão na memória (para teste e para o rótulo da tela).
    pub fn len(&self) -> usize {
        self.loaded.len()
    }

    /// O cache está vazio?
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(nome: &str) -> Schema {
        Schema {
            tables: vec![mustel_core::usecases::schema::Table {
                schema: "public".to_string(),
                name: nome.to_string(),
                kind: mustel_core::usecases::schema::RelationKind::Table,
                columns: Vec::new(),
            }],
            functions: Vec::new(),
        }
    }

    /// IF-202 e IF-203: o cache guarda por servidor **e** banco — o mesmo servidor pode ter bancos diferentes.
    /// CONTRACT: IF-202
    /// CONTRACT: IF-203
    #[test]
    fn the_cache_is_keyed_by_server_and_database() {
        let mut cache = SchemaCache::default();
        cache.insert("dev", "db1", schema("t1"));
        cache.insert("dev", "db2", schema("t2"));

        assert_eq!(cache.len(), 2, "dois bancos, duas entradas");
        assert_eq!(cache.get("dev", "db1").expect("db1").tables[0].name, "t1");
        assert_eq!(cache.get("dev", "db2").expect("db2").tables[0].name, "t2");
        assert!(cache.get("dev", "db3").is_none(), "banco não carregado");
    }

    /// Invalidar remove só o par pedido.
    /// CONTRACT: IF-203
    #[test]
    fn invalidating_one_pair_keeps_the_others() {
        let mut cache = SchemaCache::default();
        cache.insert("dev", "db1", schema("t1"));
        cache.insert("dev", "db2", schema("t2"));

        cache.invalidate("dev", "db1");

        assert!(cache.get("dev", "db1").is_none());
        assert!(cache.get("dev", "db2").is_some());
        assert_eq!(cache.len(), 1);
    }

    /// Recarregar substitui o esquema do mesmo par, sem duplicar.
    /// CONTRACT: IF-203
    #[test]
    fn reloading_replaces_the_entry() {
        let mut cache = SchemaCache::default();
        cache.insert("dev", "db1", schema("antigo"));
        cache.insert("dev", "db1", schema("novo"));

        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.get("dev", "db1").expect("carregado").tables[0].name,
            "novo"
        );
    }

    /// Servidor removido do cadastro leva junto os esquemas dele.
    ///
    /// CONTRACT: IF-216
    #[test]
    fn forgetting_a_server_drops_only_its_schemas() {
        let mut cache = SchemaCache::default();
        cache.insert("Local", "dev", schema("t1"));
        cache.insert("Local", "outro", schema("t2"));
        cache.insert("outro-servidor", "dev", schema("t3"));

        cache.forget_server("Local");

        assert!(cache.get("Local", "dev").is_none(), "o servidor saiu");
        assert!(
            cache.get("Local", "outro").is_none(),
            "todos os bancos dele"
        );
        assert!(
            cache.get("outro-servidor", "dev").is_some(),
            "os outros servidores ficam"
        );
        assert_eq!(cache.len(), 1);
    }
}
