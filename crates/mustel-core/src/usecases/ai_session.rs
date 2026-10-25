//! A conversa com a IA **persistida** entre reinícios do app (D21/D22).
//!
//! Fica num arquivo de **estado** (`<config>/ai/session.json`), não no `mustel.jsonc`: o config é
//! configuração (feito para ser editado à mão e ficar mínimo), e uma conversa mudaria a cada
//! mensagem. O que se guarda é só o que **não** dá para reconstruir — contexto e mensagens. O DDL e o
//! system prompt saem do cache na hora de montar o pedido.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::usecases::ai::{ChatMessage, Role};

/// A conversa e o contexto em que ela aconteceu.
///
/// O contexto (`server`/`database`) é o **pinado**: trocar a seleção no painel não muda este par, e
/// é ele que vai no prompt (D20).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSession {
    /// Servidor da sessão.
    pub server: String,
    /// Banco da sessão.
    pub database: String,
    /// `id` do perfil usado.
    pub profile_id: String,
    /// O painel estava aberto ao fechar o app?
    pub panel_open: bool,
    /// Mensagens, na ordem (só `user` e `assistant`).
    pub messages: Vec<ChatMessage>,
}

impl AiSession {
    /// A sessão está vazia (nenhuma mensagem e nenhum contexto)?
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty() && self.server.is_empty() && self.database.is_empty()
    }
}

/// Lê a sessão do disco.
///
/// Ausente, ilegível ou corrompida devolvem uma sessão vazia — abrir o app nunca pode falhar por
/// causa de um arquivo de conversa. Mensagens de `system` são **descartadas**: o prompt é remontado
/// do cache, não lido de disco.
pub fn load(path: &std::path::Path) -> AiSession {
    let Ok(text) = std::fs::read_to_string(path) else {
        return AiSession::default();
    };
    let Ok(mut session) = serde_json::from_str::<AiSession>(&text) else {
        return AiSession::default();
    };

    session
        .messages
        .retain(|message| message.role != Role::System);
    session
}

/// Grava a sessão (arquivo temporário + `rename`): um crash no meio não deixa JSON pela metade.
pub fn save(path: &std::path::Path, session: &AiSession) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(session)
        .map_err(|error| CoreError::Other(format!("Could not serialize the session: {error}")))?;

    let temp = path.with_extension("tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> AiSession {
        AiSession {
            server: "Local".to_string(),
            database: "configurador".to_string(),
            profile_id: "rapido".to_string(),
            panel_open: true,
            messages: vec![
                ChatMessage::user("vendas por mês"),
                ChatMessage::assistant("SELECT 1;"),
            ],
        }
    }

    /// Grava e lê de volta o contexto e a ordem das mensagens.
    #[test]
    fn the_session_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session.json");

        save(&path, &session()).expect("grava");
        let loaded = load(&path);

        assert_eq!(loaded, session());
        assert_eq!(loaded.messages[0].role, Role::User);
        assert_eq!(loaded.messages[1].role, Role::Assistant);
    }

    /// Arquivo ausente ou corrompido abrem vazio (nunca pânico).
    #[test]
    fn a_missing_or_broken_file_opens_empty() {
        let dir = tempfile::tempdir().expect("tempdir");

        assert_eq!(load(&dir.path().join("nada.json")), AiSession::default());

        let broken = dir.path().join("session.json");
        std::fs::write(&broken, "{ isto não é json").expect("escreve");
        assert_eq!(load(&broken), AiSession::default());
    }

    /// O arquivo **não** guarda DDL nem mensagem de sistema.
    #[test]
    fn the_file_holds_no_ddl_and_no_system_message() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session.json");

        save(&path, &session()).expect("grava");
        let text = std::fs::read_to_string(&path).expect("lê");

        assert!(!text.contains("CREATE TABLE"), "sem DDL: {text}");
        assert!(!text.contains("system"), "sem mensagem de sistema: {text}");

        // E se alguém injetar um `system` à mão, ele não sobrevive à leitura.
        let with_system = r#"{ "messages": [ { "role": "system", "content": "x" } ] }"#;
        std::fs::write(&path, with_system).expect("escreve");
        assert!(load(&path).messages.is_empty());
    }
}
