//! O que a janela sabe de cada servidor para a linha "Load autocomplete".
//!
//! A linha precisa de duas coisas que **não estão no cadastro**: os bancos que o servidor realmente
//! tem (descobertos em `pg_database`) e se o cofre guarda a senha dele. As duas chegam juntas, num
//! job só, e ficam guardadas aqui — o desenho não pode consultar o cofre a cada quadro.
//!
//! Sem descoberta (servidor fora do ar, senha recusada), a lista de bancos cai de volta para os
//! bancos do cadastro: a janela continua utilizável como antes da descoberta existir.

use std::collections::BTreeMap;

/// O resultado da descoberta de um servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// Bancos do servidor, ou o motivo de não ter dado para listar.
    pub databases: Result<Vec<String>, String>,
    /// O cofre tinha senha para este servidor?
    pub has_password: bool,
}

/// O que já se sabe por servidor, e o que está sendo descoberto agora.
#[derive(Debug, Default)]
pub struct Reference {
    known: BTreeMap<String, Known>,
    pending: Vec<String>,
}

impl Reference {
    /// A descoberta deste servidor ainda não foi pedida?
    ///
    /// É o gatilho do pedido: a linha pergunta uma vez por servidor.
    pub fn unknown(&self, server: &str) -> bool {
        !server.is_empty() && !self.known.contains_key(server) && !self.is_pending(server)
    }

    /// A descoberta deste servidor está em andamento?
    pub fn is_pending(&self, server: &str) -> bool {
        self.pending.iter().any(|name| name == server)
    }

    /// Marca que a descoberta foi pedida.
    pub fn mark_pending(&mut self, server: &str) {
        if !self.is_pending(server) {
            self.pending.push(server.to_string());
        }
    }

    /// Guarda o resultado da descoberta e tira o servidor da lista de pendentes.
    pub fn finish(&mut self, server: &str, known: Known) {
        self.pending.retain(|name| name != server);
        self.known.insert(server.to_string(), known);
    }

    /// Esquece um servidor (a senha ou o cadastro mudou; é preciso descobrir de novo).
    pub fn forget(&mut self, server: &str) {
        self.pending.retain(|name| name != server);
        self.known.remove(server);
    }

    /// O cofre tem senha para este servidor? `None` = ainda não se sabe.
    pub fn has_password(&self, server: &str) -> Option<bool> {
        self.known.get(server).map(|known| known.has_password)
    }

    /// Por que a descoberta não deu certo, quando não deu.
    pub fn error(&self, server: &str) -> Option<&str> {
        match self.known.get(server).map(|known| &known.databases) {
            Some(Err(error)) => Some(error.as_str()),
            _ => None,
        }
    }

    /// Bancos para o combo: os **descobertos**; sem descoberta, os do cadastro.
    ///
    /// Descoberta que voltou vazia também cai para o cadastro — um servidor sem banco acessível não
    /// deve apagar o que o usuário cadastrou.
    pub fn choices(&self, server: &str, configured: &[String]) -> Vec<String> {
        match self.known.get(server).map(|known| &known.databases) {
            Some(Ok(databases)) if !databases.is_empty() => databases.clone(),
            _ => configured.to_vec(),
        }
    }

    /// O `⟳` pode?
    ///
    /// Só falta algo quando o servidor ou o banco não foram escolhidos, ou quando o cofre **sabemos**
    /// que não tem senha (D10). Enquanto não se sabe, deixa tentar: o erro do cofre é claro e é
    /// melhor do que um botão que parece quebrado.
    pub fn can_load(&self, server: &str, database: &str) -> bool {
        !server.is_empty() && !database.is_empty() && self.has_password(server) != Some(false)
    }

    /// O que dizer no tooltip do `⟳` quando ele está desabilitado.
    /// Por que o botão de carregar está desabilitado (mensagem do **hover**), em inglês como o
    /// resto da janela.
    pub fn blocked_reason(&self, server: &str, database: &str) -> Option<&'static str> {
        if server.is_empty() || database.is_empty() {
            return Some("Pick a server and a database to load the schema");
        }
        if self.has_password(server) == Some(false) {
            return Some(
                "No password in the vault for this server: store it with `mustel settings db-servers set-password`",
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured() -> Vec<String> {
        vec!["dev".to_string()]
    }

    fn discovered(names: &[&str]) -> Known {
        Known {
            databases: Ok(names.iter().map(|name| name.to_string()).collect()),
            has_password: true,
        }
    }

    /// O combo mostra os bancos do servidor, não só os do cadastro.
    ///
    /// CONTRACT: IF-211
    #[test]
    fn the_combo_lists_the_databases_found_on_the_server() {
        let mut reference = Reference::default();
        reference.finish("local", discovered(&["dev", "postgres", "test"]));

        assert_eq!(
            reference.choices("local", &configured()),
            vec![
                "dev".to_string(),
                "postgres".to_string(),
                "test".to_string()
            ]
        );
    }

    /// Sem descoberta (ou com erro), vale o cadastro.
    ///
    /// CONTRACT: IF-211
    #[test]
    fn without_discovery_the_configured_databases_are_used() {
        let mut reference = Reference::default();
        assert_eq!(reference.choices("local", &configured()), configured());

        reference.finish(
            "local",
            Known {
                databases: Err("connection refused".to_string()),
                has_password: true,
            },
        );
        assert_eq!(reference.choices("local", &configured()), configured());

        // Descoberta vazia não apaga o que o usuário cadastrou.
        reference.finish("local", discovered(&[]));
        assert_eq!(reference.choices("local", &configured()), configured());
    }

    /// A linha pede a descoberta uma vez por servidor.
    #[test]
    fn each_server_is_discovered_once() {
        let mut reference = Reference::default();
        assert!(reference.unknown("local"), "nada se sabe ainda");

        reference.mark_pending("local");
        assert!(!reference.unknown("local"), "já foi pedido");
        assert!(reference.is_pending("local"));

        reference.finish("local", discovered(&["dev"]));
        assert!(!reference.unknown("local"));
        assert!(!reference.is_pending("local"));
    }

    /// O `⟳` fica desabilitado quando o cofre não tem senha (D10).
    ///
    /// CONTRACT: IF-212
    #[test]
    fn the_button_is_disabled_without_a_stored_password() {
        let mut reference = Reference::default();

        // Ainda não se sabe: deixa tentar (o erro do cofre é claro).
        assert!(reference.can_load("local", "dev"));

        reference.finish(
            "local",
            Known {
                databases: Err("No password stored for 'local'.".to_string()),
                has_password: false,
            },
        );
        assert!(!reference.can_load("local", "dev"), "sem senha não carrega");
        assert!(reference.blocked_reason("local", "dev").is_some());

        // Guardar a senha esquece o servidor e o botão volta a poder.
        reference.forget("local");
        assert!(reference.can_load("local", "dev"));
    }

    /// Sem servidor ou sem banco escolhido também não carrega.
    #[test]
    fn the_button_needs_a_server_and_a_database() {
        let reference = Reference::default();
        assert!(!reference.can_load("", "dev"));
        assert!(!reference.can_load("local", ""));
        assert!(reference.can_load("local", "dev"));
    }

    /// O motivo aparece só quando o botão está de fato bloqueado.
    ///
    /// CONTRACT: IF-212
    #[test]
    fn the_blocked_reason_matches_the_button_state() {
        let reference = Reference::default();
        assert!(reference.blocked_reason("", "dev").is_some());
        assert!(reference.blocked_reason("local", "").is_some());
        assert!(reference.blocked_reason("local", "dev").is_none());
    }
}
