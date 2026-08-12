//! HTTP for the two Live endpoints, wrapped around `ureq` configured for
//! `native-tls` so TLS stays on the same Security.framework stack the
//! Battle.net socket uses. Redirects are disabled on purpose: the endpoint
//! must be the final URL, and a redirected POST would re-send the payload to
//! a host nobody vetted.

use std::time::Duration;

use ureq::Agent;
use ureq::tls::{TlsConfig, TlsProvider};

/// Response bodies larger than this are a config error, not data.
const MAX_RESPONSE_BYTES: u64 = 65_536;

/// How a POST failed, which decides what the worker does next.
#[derive(Debug)]
pub enum PostError {
    /// Transient: network trouble, 408/429, 5xx. Retry the same payload.
    Retryable(String),
    /// The server rejected this payload (other 4xx). Drop it and move on.
    Rejected(String),
    /// 401/403: the token is bad or revoked. Latch off until restart.
    Unauthorized,
    /// Configuration problems (redirects, bad URLs). Retrying cannot help.
    Fatal(String),
}

/// A parsed response: the status code and the (bounded) body.
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// The endpoint origin must be `https://`, except loopback for `wrangler dev`.
pub fn validate_endpoint(base: &str) -> Result<(), String> {
    let url = url::Url::parse(base).map_err(|error| format!("bad endpoint {base}: {error}"))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = url.host_str().unwrap_or("");
            if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
                Ok(())
            } else {
                Err("plain http endpoints are only allowed toward loopback".into())
            }
        }
        other => Err(format!("unsupported endpoint scheme {other}")),
    }
}

pub struct LiveHttp {
    agent: Agent,
}

impl LiveHttp {
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(15))
    }

    /// Shrunken timeout for the shutdown flush, which runs on a deadline.
    #[must_use]
    pub fn brief() -> Self {
        Self::with_timeout(Duration::from_secs(4))
    }

    fn with_timeout(timeout: Duration) -> Self {
        let config = Agent::config_builder()
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .build(),
            )
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.new_agent(),
        }
    }

    /// POSTs a JSON body and maps the outcome onto the retry taxonomy. Any
    /// 2xx returns the response for the caller to parse.
    pub fn post_json(
        &self,
        url: &str,
        token: Option<&str>,
        body: &[u8],
    ) -> Result<HttpResponse, PostError> {
        let mut request = self.agent.post(url).content_type("application/json");
        if let Some(token) = token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let mut response = request
            .send(body)
            .map_err(|error| PostError::Retryable(format!("send: {error}")))?;

        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|error| PostError::Retryable(format!("read: {error}")))?;

        match status {
            200..=299 => Ok(HttpResponse { status, body }),
            401 | 403 => Err(PostError::Unauthorized),
            300..=399 => Err(PostError::Fatal(format!(
                "endpoint redirects (HTTP {status}); use the final URL"
            ))),
            408 | 429 => Err(PostError::Retryable(format!("HTTP {status}"))),
            400..=499 => Err(PostError::Rejected(format!(
                "HTTP {status}: {}",
                String::from_utf8_lossy(&body[..body.len().min(200)])
            ))),
            _ => Err(PostError::Retryable(format!("HTTP {status}"))),
        }
    }
}

impl Default for LiveHttp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn serve_once(response: &'static str) -> (u16, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut received = vec![0_u8; 16_384];
            let mut total = 0;
            loop {
                let read = stream.read(&mut received[total..]).expect("read");
                total += read;
                if read == 0 || request_complete(&received[..total]) {
                    break;
                }
            }
            received.truncate(total);
            stream.write_all(response.as_bytes()).expect("write");
            received
        });
        (port, handle)
    }

    fn request_complete(raw: &[u8]) -> bool {
        let Some(head_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let head = String::from_utf8_lossy(&raw[..head_end]);
        let content_length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        raw.len() >= head_end + 4 + content_length
    }

    #[test]
    fn posts_headers_and_body_and_reads_the_response() {
        let (port, server) = serve_once(
            "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"accepted\":5}",
        );
        let response = LiveHttp::new()
            .post_json(
                &format!("http://127.0.0.1:{port}/v1/events"),
                Some("secret-token"),
                b"{\"v\":1}",
            )
            .expect("post");
        assert_eq!(response.status, 202);
        assert_eq!(response.body, b"{\"accepted\":5}");

        let request = String::from_utf8(server.join().expect("server")).expect("utf8");
        assert!(request.starts_with("POST /v1/events HTTP/1.1\r\n"));
        assert!(request.contains("authorization: Bearer secret-token\r\n"));
        assert!(request.contains("content-type: application/json\r\n"));
        assert!(request.ends_with("{\"v\":1}"));
    }

    type ErrorMatcher = fn(&PostError) -> bool;

    #[test]
    fn maps_statuses_onto_the_retry_taxonomy() {
        let cases: [(&'static str, ErrorMatcher); 4] = [
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                |error| matches!(error, PostError::Unauthorized),
            ),
            (
                "HTTP/1.1 500 Oops\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                |error| matches!(error, PostError::Retryable(_)),
            ),
            (
                "HTTP/1.1 303 See Other\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                |error| matches!(error, PostError::Fatal(_)),
            ),
            (
                "HTTP/1.1 400 Bad\r\nContent-Length: 9\r\nConnection: close\r\n\r\nbad batch",
                |error| matches!(error, PostError::Rejected(message) if message.contains("bad batch")),
            ),
        ];
        for (response, matches_expected) in cases {
            let (port, _server) = serve_once(response);
            let error = LiveHttp::new()
                .post_json(&format!("http://127.0.0.1:{port}/v1/events"), None, b"{}")
                .expect_err("should map to an error");
            assert!(matches_expected(&error), "{response}: got {error:?}");
        }
    }

    #[test]
    fn endpoint_validation_requires_https_except_loopback() {
        assert!(validate_endpoint("https://live.example.com").is_ok());
        assert!(validate_endpoint("http://127.0.0.1:8787").is_ok());
        assert!(validate_endpoint("http://localhost:8787").is_ok());
        assert!(validate_endpoint("http://live.example.com").is_err());
        assert!(validate_endpoint("ftp://live.example.com").is_err());
        assert!(validate_endpoint("not a url").is_err());
    }
}
