//! O transporte HTTP do updater, atrás de um **traço** (como o da IA).
//!
//! A rede é a única parte não testável sem socket; o `check` e o `download` recebem um
//! [`UpdateTransport`], então a suíte usa um falso e o binário usa [`ReqwestTransport`].

use std::path::Path;
use std::time::Duration;

use crate::error::CoreError;

/// Resposta crua de um `GET` (status e corpo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Status HTTP.
    pub status: u16,
    /// Corpo da resposta.
    pub body: Vec<u8>,
}

/// Quem sabe fazer `GET` e baixar um arquivo com progresso.
pub trait UpdateTransport: Send + Sync {
    /// `GET` simples (JSON da API e `SHA256SUMS`).
    fn get(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<HttpResponse, CoreError>> + Send;

    /// Baixa `url` para `into`, chamando `progress(done, total)`.
    ///
    /// O callback devolve `false` para **cancelar**; nesse caso o arquivo parcial fica para o
    /// chamador apagar e o erro é [`CoreError::Cancelled`].
    fn download<'a>(
        &'a self,
        url: &'a str,
        into: &'a Path,
        progress: &'a mut (dyn FnMut(u64, Option<u64>) -> bool + Send),
    ) -> impl std::future::Future<Output = Result<(), CoreError>> + Send + 'a;
}

/// Timeout do `GET` da API/checksum.
const GET_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout do download do executável.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
/// Timeout de conexão.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Quantos redirects o download do asset pode seguir (o GitHub manda para
/// `objects.githubusercontent.com`).
const MAX_REDIRECTS: usize = 5;

/// O transporte de verdade, sobre `reqwest` com **rustls**.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Cliente com timeouts explícitos e `User-Agent` (a API do GitHub exige um).
    pub fn new() -> Result<Self, CoreError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("mustel/{}", env!("CARGO_PKG_VERSION")))
            .timeout(GET_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .map_err(|error| {
                CoreError::Other(format!("Could not build the update client: {error}"))
            })?;

        Ok(Self { client })
    }
}

impl UpdateTransport for ReqwestTransport {
    async fn get(&self, url: &str) -> Result<HttpResponse, CoreError> {
        let response = self
            .client
            .get(url)
            .header("accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|error| CoreError::Other(format!("Could not reach {url}: {error}")))?;

        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|error| CoreError::Other(format!("Could not read {url}: {error}")))?
            .to_vec();

        Ok(HttpResponse { status, body })
    }

    async fn download<'a>(
        &'a self,
        url: &'a str,
        into: &'a Path,
        progress: &'a mut (dyn FnMut(u64, Option<u64>) -> bool + Send),
    ) -> Result<(), CoreError> {
        let mut response = self
            .client
            .get(url)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await
            .map_err(|error| CoreError::Other(format!("Could not download the update: {error}")))?;

        if !response.status().is_success() {
            return Err(CoreError::Other(format!(
                "The update download returned HTTP {}",
                response.status()
            )));
        }

        let total = response.content_length();
        let mut file = tokio::fs::File::create(into).await.map_err(|error| {
            CoreError::Other(format!("Could not create the update file: {error}"))
        })?;
        let mut done = 0_u64;

        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| CoreError::Other(format!("Could not read the update: {error}")))?
        {
            file.write_all(&chunk).await.map_err(|error| {
                CoreError::Other(format!("Could not write the update: {error}"))
            })?;
            done += chunk.len() as u64;
            if !progress(done, total) {
                return Err(CoreError::Cancelled);
            }
        }

        file.flush().await.map_err(|error| {
            CoreError::Other(format!("Could not finish writing the update: {error}"))
        })?;
        Ok(())
    }
}
