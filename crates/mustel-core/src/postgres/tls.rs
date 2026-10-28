//! Modos de SSL e a configuração TLS do driver.
//!
//! O `sslMode` do produto tem os três valores do `tokio-postgres`: `Disable`, `Prefer` e
//! `Require`. Não há `Allow`, `VerifyCA` nem `VerifyFull`: TLS, quando usado, sempre valida a
//! cadeia e o hostname com o verificador padrão do rustls. A confiança vem do store do sistema
//! (`rustls-native-certs`); certificado self-signed exige a CA instalada no SO.
//!
//! `Prefer` é oportunista: tenta TLS e só cai para texto puro quando o servidor responde que não
//! suporta TLS. Erro de certificado/handshake **não** vira texto puro; `Require` nunca cai.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use rustls::ClientConfig;
use rustls::RootCertStore;
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::error::CoreError;

/// Modo de SSL inválido.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid ssl mode: '{0}'. Use Disable, Prefer or Require. Allow, VerifyCA and VerifyFull are \
     no longer supported: use Prefer (TLS first) or Disable instead of Allow, and Require instead \
     of VerifyCA/VerifyFull."
)]
pub struct SslModeError(pub String);

/// Modo de SSL pedido pelo usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SslMode {
    /// Sem TLS.
    Disable,
    /// Tenta TLS; se o servidor não suportar, segue sem (padrão).
    #[default]
    Prefer,
    /// Exige TLS com validação de cadeia e hostname.
    Require,
}

impl SslMode {
    /// Exige TLS?
    pub fn requires_tls(self) -> bool {
        matches!(self, Self::Require)
    }

    /// Pode usar TLS (mesmo que não o exija)?
    pub fn uses_tls(self) -> bool {
        matches!(self, Self::Prefer | Self::Require)
    }

    /// Nome como o produto escreve.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "Disable",
            Self::Prefer => "Prefer",
            Self::Require => "Require",
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
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            other => Err(SslModeError(other.to_string())),
        }
    }
}

/// Cria o conector rustls usado por `Prefer`/`Require`.
///
/// O `RootCertStore` vem do store nativo do sistema. Store vazio é erro: não faz sentido tentar TLS
/// sem nenhuma âncora de confiança, e cair para texto puro seria um downgrade silencioso.
pub fn tls_connector() -> Result<MakeRustlsConnect, CoreError> {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = RootCertStore::empty();
    let (added, _) = roots.add_parsable_certificates(native.certs);

    if added == 0 {
        return Err(CoreError::InvalidState(format!(
            "no trusted CA certificates were loaded from the system store: {:?}",
            native.errors
        )));
    }

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| {
            CoreError::InvalidState(format!(
                "could not configure TLS protocol versions: {error}"
            ))
        })?
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(MakeRustlsConnect::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CONTRACT: IF-021
    #[test]
    fn parses_the_three_supported_names_ignoring_case() {
        assert_eq!("Disable".parse::<SslMode>().expect("ok"), SslMode::Disable);
        assert_eq!("prefer".parse::<SslMode>().expect("ok"), SslMode::Prefer);
        assert_eq!("REQUIRE".parse::<SslMode>().expect("ok"), SslMode::Require);
        assert_eq!(
            "Prefer".parse::<SslMode>().expect("ok").to_string(),
            "Prefer"
        );
    }

    #[test]
    /// CONTRACT: IF-256
    fn removed_and_unknown_modes_are_rejected_with_migration() {
        for mode in ["Allow", "VerifyCA", "VerifyFull", "verify-ca", "maybe"] {
            let error = mode.parse::<SslMode>().expect_err("deve recusar");
            assert!(error.to_string().contains("no longer supported"), "{error}");
            assert!(error.to_string().contains("Prefer"), "{error}");
        }
    }

    #[test]
    fn tls_requirements_follow_the_three_modes() {
        assert!(!SslMode::Disable.requires_tls());
        assert!(!SslMode::Prefer.requires_tls());
        assert!(SslMode::Require.requires_tls());

        assert!(!SslMode::Disable.uses_tls());
        assert!(SslMode::Prefer.uses_tls());
        assert!(SslMode::Require.uses_tls());
    }
}
