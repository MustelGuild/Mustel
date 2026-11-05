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
    /// Cliente com timeouts explícitos (nenhum infinito) e **sem seguir redirect**.
    pub fn new() -> Result<Self, CoreError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            // O `baseUrl` cadastrado é o único destino aceito: seguir um 307/308 reenviaria o
            // prompt e o DDL a um host que ninguém validou, e um downgrade para `http` os
            // mandaria em texto puro.
            .redirect(reqwest::redirect::Policy::none())
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

        // Sem política de redirect, o `reqwest` devolveria o 3xx em silêncio; aqui ele vira erro
        // para o usuário saber que o endpoint tentou levá-lo a outro destino.
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok());
            return Err(CoreError::Other(format!(
                "The AI endpoint redirected to {}; configure the final URL in the profile.",
                redirect_destination(location)
            )));
        }

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| CoreError::Other(format!("Could not read the reply: {error}")))?;

        Ok(HttpReply { status, body })
    }
}

/// Descreve o destino de um redirect sem repassar ao terminal texto arbitrário do servidor.
///
/// Só esquema, host e porta: o `Location` é controlado por quem respondeu, e imprimi-lo inteiro
/// permitiria sequências de controle na saída do CLI.
fn redirect_destination(location: Option<&str>) -> String {
    let Some(location) = location else {
        return "a URL with no Location header".to_string();
    };

    match url::Url::parse(location) {
        Ok(url) => match (url.host_str(), url.port()) {
            (Some(host), Some(port)) => format!("{}://{host}:{port}", url.scheme()),
            (Some(host), None) => format!("{}://{host}", url.scheme()),
            _ => "a URL with no host".to_string(),
        },
        Err(_) => "an unparseable URL".to_string(),
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

    /// O transporte não segue redirect: um 307/308 reenviaria prompt e DDL a outro host.
    #[tokio::test]
    async fn a_redirect_is_not_followed() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        // Destino do redirect: se o cliente seguisse, abriria conexão aqui, e a resposta 200
        // provaria o salto em vez de deixar o teste pendurado.
        let sink = TcpListener::bind("127.0.0.1:0").expect("bind do destino");
        sink.set_nonblocking(true).expect("modo não bloqueante");
        let sink_address = sink.local_addr().expect("endereço do destino");
        let stop = Arc::new(AtomicBool::new(false));
        let sink_hit = Arc::new(AtomicBool::new(false));
        let sink_stop = Arc::clone(&stop);
        let sink_flag = Arc::clone(&sink_hit);
        let sink_server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if sink_stop.load(Ordering::SeqCst) || Instant::now() >= deadline {
                    return;
                }
                match sink.accept() {
                    Ok((mut socket, _)) => {
                        sink_flag.store(true, Ordering::SeqCst);
                        let mut buffer = [0_u8; 2048];
                        let _ = socket.read(&mut buffer);
                        let _ = socket.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        );
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept do destino: {error}"),
                }
            }
        });

        // Servidor que responde 307 apontando para o destino.
        let redirector = TcpListener::bind("127.0.0.1:0").expect("bind do servidor");
        let redirector_address = redirector.local_addr().expect("endereço do servidor");
        let server = std::thread::spawn(move || {
            redirector
                .set_nonblocking(true)
                .expect("modo não bloqueante");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let (mut socket, _) = loop {
                match redirector.accept() {
                    Ok(pair) => break pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "o cliente não conectou no servidor de teste"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };

            let mut buffer = [0_u8; 2048];
            let _ = socket.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{sink_address}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = socket.write_all(response.as_bytes());
        });

        let transport = ReqwestTransport::new().expect("cliente");
        let url = format!("http://{redirector_address}/v1/chat");
        let error = tokio::time::timeout(
            Duration::from_secs(15),
            transport.post_json(&url, "sk-test", "{}".to_string()),
        )
        .await
        .expect("a chamada não pode pendurar")
        .expect_err("um redirect precisa falhar, não ser seguido");

        stop.store(true, Ordering::SeqCst);
        sink_server.join().expect("servidor do destino");
        server.join().expect("servidor de teste");

        let message = error.to_string();
        assert!(message.contains("redirected to"), "{message}");
        assert!(message.contains("127.0.0.1"), "{message}");
        assert!(
            !sink_hit.load(Ordering::SeqCst),
            "o cliente seguiu o redirect e conectou no destino"
        );
    }
}
