//! Modos de SSL aceitos pelo legado (Npgsql) e sua tradução para o driver.
//!
//! O `sslMode` do FurLab é o de Npgsql: `Disable`, `Allow`, `Prefer`, `Require`, `VerifyCA`,
//! `VerifyFull`. O `tokio-postgres` não tem esses nomes: quem decide sobre TLS é o tipo do
//! conector (`NoTls` ou um `MakeTlsConnect`).
//!
//! Estado atual (T19, parcial): a tradução **está definida e testada**, mas a conexão real com TLS
//! ainda não foi ligada — os modos que exigem TLS devolvem erro claro em vez de degradar para
//! texto puro silenciosamente. Falta ligar `tokio-postgres-rustls` com os certificados do sistema e
//! implementar a diferença entre `VerifyCA` (valida a cadeia, **não** o hostname, como o Npgsql) e
//! `VerifyFull` (valida os dois, que é o padrão do rustls). Ver `docs/rust-postgres-spike.md`.

use std::fmt;
use std::str::FromStr;

/// Modo de SSL inválido.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid ssl mode: '{0}'. Use Disable, Allow, Prefer, Require, VerifyCA or VerifyFull.")]
pub struct SslModeError(pub String);

/// Modo de SSL pedido pelo usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SslMode {
    /// Sem TLS.
    Disable,
    /// Tenta TLS; se o servidor não suportar, segue sem.
    Allow,
    /// Tenta TLS; se o servidor não suportar, segue sem (padrão do legado).
    #[default]
    Prefer,
    /// Exige TLS.
    Require,
    /// Exige TLS e valida a cadeia (sem checar o hostname, como o Npgsql).
    VerifyCa,
    /// Exige TLS e valida cadeia **e** hostname.
    VerifyFull,
}

impl SslMode {
    /// Exige TLS?
    pub fn requires_tls(self) -> bool {
        matches!(self, Self::Require | Self::VerifyCa | Self::VerifyFull)
    }

    /// Tenta TLS, aceitando seguir sem quando o servidor não oferece?
    pub fn tries_tls(self) -> bool {
        !matches!(self, Self::Disable)
    }

    /// Valida o hostname do certificado? (`VerifyCA` do Npgsql **não** valida.)
    pub fn verifies_hostname(self) -> bool {
        matches!(self, Self::VerifyFull)
    }

    /// Nome como o legado escreve.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "Disable",
            Self::Allow => "Allow",
            Self::Prefer => "Prefer",
            Self::Require => "Require",
            Self::VerifyCa => "VerifyCA",
            Self::VerifyFull => "VerifyFull",
        }
    }
}

impl fmt::Display for SslMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SslMode {
    type Err = SslModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "allow" => Ok(Self::Allow),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verifyca" | "verify-ca" => Ok(Self::VerifyCa),
            "verifyfull" | "verify-full" => Ok(Self::VerifyFull),
            other => Err(SslModeError(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-021
    fn parses_every_legacy_name_ignoring_case() {
        assert_eq!("Disable".parse::<SslMode>().expect("ok"), SslMode::Disable);
        assert_eq!("prefer".parse::<SslMode>().expect("ok"), SslMode::Prefer);
        assert_eq!("REQUIRE".parse::<SslMode>().expect("ok"), SslMode::Require);
        assert_eq!(
            "verifyca".parse::<SslMode>().expect("ok"),
            SslMode::VerifyCa
        );
        assert_eq!(
            "VerifyFull".parse::<SslMode>().expect("ok"),
            SslMode::VerifyFull
        );
        assert_eq!(
            "Prefer".parse::<SslMode>().expect("ok").to_string(),
            "Prefer"
        );
    }

    #[test]
    fn unknown_mode_is_rejected() {
        assert!("maybe".parse::<SslMode>().is_err());
    }

    #[test]
    fn tls_requirements_match_npgsql_semantics() {
        assert!(!SslMode::Disable.requires_tls());
        assert!(!SslMode::Allow.requires_tls());
        assert!(!SslMode::Prefer.requires_tls());
        assert!(SslMode::Require.requires_tls());
        assert!(SslMode::VerifyCa.requires_tls());
        assert!(SslMode::VerifyFull.requires_tls());

        assert!(!SslMode::Disable.tries_tls());
        assert!(SslMode::Allow.tries_tls());
        assert!(SslMode::Prefer.tries_tls());

        // Diferença sutil que precisa ser respeitada: VerifyCA valida a cadeia sem checar hostname.
        assert!(!SslMode::VerifyCa.verifies_hostname());
        assert!(SslMode::VerifyFull.verifies_hostname());
    }
}
