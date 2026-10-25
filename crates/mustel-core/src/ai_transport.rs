//! O transporte HTTP da IA, atrás de um **traço**.
//!
//! A chamada ao provedor é a única parte que toca a rede; colocá-la atrás de [`HttpTransport`]
//! deixa o resto do caminho (montagem do pedido, leitura da resposta, mensagens de erro) testável
//! sem socket — a mesma ideia do `pg_dump` injetável em [`crate::usecases::ddl`].

use std::time::Duration;

use crate::error::CoreError;

/// Resposta crua do provedor: código e corpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReply {
    /// Status HTTP.
    pub status: u16,
    /// Corpo da resposta (JSON, no caso feliz).
    pub body: String,
}

/// Quem sabe fazer um `POST` com `Bearer`.
pub trait HttpTransport: Send + Sync {
    /// Envia o corpo e devolve a resposta **crua** (inclusive quando o status é de erro).
    fn post_json(
        &self,
        url: &str,
        bearer: &str,
        body: String,
    ) -> impl std::future::Future<Output = Result<HttpReply, CoreError>> + Send;
}

/// Timeout total de uma chamada (o modelo pode demorar).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Timeout de conexão.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// O transporte de verdade, sobre `reqwest` com **rustls** (sem OpenSSL).
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Cliente com timeouts explícitos (nenhum infinito).
    pub fn new() -> Result<Self, CoreError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|error| {
                CoreError::Other(format!("Could not build the HTTP client: {error}"))
            })?;
        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestTransport {
    async fn post_json(
        &self,
        url: &str,
        bearer: &str,
        body: String,
    ) -> Result<HttpReply, CoreError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(bearer)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| CoreError::Other(format!("Could not reach {url}: {error}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| CoreError::Other(format!("Could not read the reply: {error}")))?;

        Ok(HttpReply { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O cliente real é construível (rustls, sem OpenSSL e sem rede).
    #[test]
    fn the_client_builds() {
        assert!(ReqwestTransport::new().is_ok());
    }
}
