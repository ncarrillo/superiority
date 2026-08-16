#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    time::{Duration, Instant},
};

use sc2_core::{Error, Result, bgs::SecretBytes};
use url::Url;

const MAX_CREDENTIAL_BYTES: usize = 1024;
const REQUEST_POLL: Duration = Duration::from_millis(50);

pub type Cancellation = Arc<dyn Fn() -> bool + Send + Sync>;

pub struct Request {
    url: Url,
    timeout: Duration,
    deadline: Instant,
    cancellation: Cancellation,
    reply: Sender<Result<SecretBytes>>,
}

#[derive(Clone)]
pub struct Requester(Sender<Request>);

pub fn channel() -> (Requester, Receiver<Request>) {
    let (sender, receiver) = mpsc::channel();
    (Requester(sender), receiver)
}

impl Requester {
    pub fn request(
        &self,
        url: &Url,
        timeout: Duration,
        cancellation: &Cancellation,
    ) -> Result<SecretBytes> {
        let (reply, response) = mpsc::channel();
        let deadline = Instant::now() + timeout;
        self.0
            .send(Request {
                url: url.clone(),
                timeout,
                deadline,
                cancellation: Arc::clone(cancellation),
                reply,
            })
            .map_err(|_| Error::Authentication("authentication interface stopped".into()))?;
        loop {
            if cancellation() {
                return Err(cancelled());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(timed_out(timeout));
            }
            match response.recv_timeout(remaining.min(REQUEST_POLL)) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::Authentication(
                        "authentication interface stopped".into(),
                    ));
                }
            }
        }
    }
}

pub fn process_pending(requests: &Receiver<Request>) {
    while let Ok(request) = requests.try_recv() {
        request.fulfill();
    }
}

impl Request {
    fn fulfill(self) {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let result = if (self.cancellation)() {
            Err(cancelled())
        } else if remaining.is_zero() {
            Err(timed_out(self.timeout))
        } else {
            request_credential(&self.url, remaining, &self.cancellation)
        };
        let _ = self.reply.send(result);
    }
}

fn cancelled() -> Error {
    Error::Authentication("Battle.net login cancelled because the legacy connection closed".into())
}

fn timed_out(timeout: Duration) -> Error {
    Error::Authentication(format!(
        "Battle.net web login timed out after {} seconds",
        timeout.as_secs()
    ))
}

/// presents battle.net's login page and intercepts its one-time localhost:0
/// credential callback; on macos this must run on the main thread.
fn request_credential(
    url: &Url,
    timeout: Duration,
    cancellation: &Cancellation,
) -> Result<SecretBytes> {
    #[cfg(target_os = "macos")]
    {
        eprintln!("Opening the Battle.net login window…");
        macos::request_credential(url, timeout, cancellation)
    }

    #[cfg(target_os = "windows")]
    {
        eprintln!("Opening the Battle.net login window…");
        windows::request_credential(url, timeout, cancellation)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (url, timeout, cancellation);
        Err(Error::Authentication(
            "the embedded Battle.net login window requires macOS or Windows".into(),
        ))
    }
}

fn parse_credential_redirect(location: &str) -> Result<SecretBytes> {
    let parsed = Url::parse(location.trim())
        .map_err(|_| Error::Authentication("web credential redirect does not parse".into()))?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("localhost")
        || parsed.port() != Some(0)
    {
        return Err(Error::Authentication(
            "web credential redirect is not the localhost:0 callback".into(),
        ));
    }

    let mut values = parsed
        .query_pairs()
        .filter(|(name, _)| name == "ST")
        .map(|(_, value)| value.into_owned());
    let value = values
        .next()
        .ok_or_else(|| Error::Authentication("web credential redirect has no ST value".into()))?;
    if values.next().is_some() {
        return Err(Error::Authentication(
            "web credential redirect has more than one ST value".into(),
        ));
    }
    let bytes = value.into_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_CREDENTIAL_BYTES
        || !bytes.iter().all(u8::is_ascii_graphic)
    {
        return Err(Error::Authentication(
            "web credential has an invalid length or characters".into(),
        ));
    }
    SecretBytes::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_the_retail_localhost_zero_callback() {
        let credential =
            parse_credential_redirect("http://localhost:0/?ST=US-1234-abcd").expect("valid");
        assert_eq!(credential.expose(), b"US-1234-abcd");
        assert!(parse_credential_redirect("http://localhost:8080/?ST=US-1").is_err());
        assert!(parse_credential_redirect("https://localhost:0/?ST=US-1").is_err());
        assert!(parse_credential_redirect("http://example.com:0/?ST=US-1").is_err());
        assert!(parse_credential_redirect("http://localhost:0/?ST=").is_err());
        assert!(parse_credential_redirect("http://localhost:0/?ST=a&ST=b").is_err());
    }

    #[test]
    fn requester_stops_waiting_when_its_legacy_connection_closes() {
        let (requester, requests) = channel();
        let cancellation: Cancellation = Arc::new(|| true);
        let url = Url::parse("https://example.invalid/login").expect("valid url");

        let error = requester
            .request(&url, Duration::from_secs(1), &cancellation)
            .expect_err("closed connection must cancel authentication");
        process_pending(&requests);

        assert!(error.to_string().contains("legacy connection closed"));
    }

    #[test]
    fn expired_queued_request_does_not_open_a_stale_login_window() {
        let (requester, requests) = channel();
        let cancellation: Cancellation = Arc::new(|| false);
        let url = Url::parse("https://example.invalid/login").expect("valid url");

        let error = requester
            .request(&url, Duration::ZERO, &cancellation)
            .expect_err("zero duration must time out");
        process_pending(&requests);

        assert!(error.to_string().contains("timed out"));
    }
}
