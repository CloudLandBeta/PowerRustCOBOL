// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The sign-in a connection's profile names (spec 103 R6.2, R7, R9, R11).
//!
//! When the program has set no keys, a request is signed with what the AWS
//! credential providers find — the AWS CLI's own sources, read by the official
//! `aws-config` crate: the shared credentials and config files (static keys,
//! `credential_process`, IAM Identity Center, `aws login`, role assumption),
//! the environment, and an instance role.
//!
//! - **A named profile wins over the environment.** The connection's profile is
//!   an explicit choice, so `AWS_ACCESS_KEY_ID` in the application's environment
//!   does not override it (the Python route this replaces behaved so too). With
//!   no profile, the default chain runs, environment first.
//! - **One HTTP stack** (R9). `aws-config` is built without an HTTP client of its
//!   own; [`UreqClient`] gives its providers the `ureq` + operating-system-TLS
//!   stack the rest of this runtime uses, and nothing else is linked.
//! - **Async stays here.** The providers are async; the AWS controls are not. One
//!   private runtime serves credential lookups and nothing else, entered from
//!   the calling thread by a channel, so a call is bounded by its own budget.
//! - **Refreshed when it matters** (R3). A credential is kept until a minute
//!   before it expires; one without an expiry is kept for the life of the link.
//!   A failure is never kept: the next request asks again, so a sign-in the user
//!   renews in a terminal is picked up without restarting anything.
//! - **Never printed.** Nothing here formats a credential; a failure is reported
//!   with the provider's own words, which name files and profiles, not keys.

use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_config::provider_config::ProviderConfig;
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::provider::error::CredentialsError;
use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};
use aws_credential_types::Credentials;
use aws_smithy_runtime_api::client::http::{
    http_client_fn, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpClient, SharedHttpConnector,
};
use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
use aws_smithy_runtime_api::client::result::ConnectorError;
use aws_smithy_runtime_api::http::StatusCode;
use aws_smithy_types::body::SdkBody;

use crate::http_runtime::{self, SendError};

/// How long before a credential's expiry it is asked for again.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// How long a provider's own request (SSO, STS, the sign-in service) may take
/// when the SDK does not say.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Why no credential could be had.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChainError {
    /// The time allowed ran out.
    Timeout,
    /// Nothing signed in answered; the words are the provider's own and name no key.
    Failed(String),
}

/// The private runtime for credential lookups.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("aws-credentials")
            .enable_all()
            .build()
            .expect("a tokio runtime for credential lookups")
    })
}

/// Which providers answer for a connection, and what they last found.
pub(crate) struct Chain {
    profile: Option<String>,
    region: Option<String>,
    /// Built on first use, on the runtime.
    provider: OnceLock<SharedCredentialsProvider>,
    found: Mutex<Option<Credentials>>,
}

impl Chain {
    /// A chain for `profile` (the default chain when none), reading AWS
    /// configuration for `region`. Nothing is read or sent yet.
    pub(crate) fn new(profile: Option<&str>, region: Option<&str>) -> Chain {
        let clean = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        Chain { profile: clean(profile), region: clean(region), provider: OnceLock::new(), found: Mutex::new(None) }
    }

    /// The credential to sign with now: the one held, or a fresh one.
    pub(crate) fn credentials(&self, timeout: Duration) -> Result<Credentials, ChainError> {
        if let Some(held) = lock(&self.found).as_ref() {
            if held.expiry().is_none_or(|e| e > SystemTime::now() + REFRESH_MARGIN) {
                return Ok(held.clone());
            }
        }
        let provider = self.provider(timeout)?;
        let (tx, rx) = mpsc::channel();
        runtime().spawn(async move {
            let _ = tx.send(provider.provide_credentials().await);
        });
        match rx.recv_timeout(timeout) {
            Ok(Ok(fresh)) => {
                *lock(&self.found) = Some(fresh.clone());
                Ok(fresh)
            }
            Ok(Err(CredentialsError::ProviderTimedOut(_))) | Err(mpsc::RecvTimeoutError::Timeout) => Err(ChainError::Timeout),
            Ok(Err(e)) => Err(ChainError::Failed(format!("Unable to locate credentials: {}", describe(&e)))),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ChainError::Failed("Unable to locate credentials: the lookup stopped".into())),
        }
    }

    /// Forget what was found, so the next request asks again.
    pub(crate) fn forget(&self) {
        *lock(&self.found) = None;
    }

    fn provider(&self, timeout: Duration) -> Result<SharedCredentialsProvider, ChainError> {
        if let Some(p) = self.provider.get() {
            return Ok(p.clone());
        }
        let (profile, region) = (self.profile.clone(), self.region.clone());
        let (tx, rx) = mpsc::channel();
        runtime().spawn(async move {
            let _ = tx.send(build_provider(profile, region).await);
        });
        let built = rx.recv_timeout(timeout).map_err(|_| ChainError::Timeout)?;
        Ok(self.provider.get_or_init(|| built).clone())
    }
}

/// The providers a connection's profile (or the default chain) stands for.
async fn build_provider(profile: Option<String>, region: Option<String>) -> SharedCredentialsProvider {
    let config = ProviderConfig::default()
        .with_http_client(ureq_client())
        .with_behavior_version(Some(BehaviorVersion::latest()))
        .with_region(region.map(Region::new));
    match profile {
        Some(name) => {
            SharedCredentialsProvider::new(ProfileFileCredentialsProvider::builder().configure(&config).profile_name(name).build())
        }
        None => SharedCredentialsProvider::new(DefaultCredentialsChain::builder().configure(config).build().await),
    }
}

// ── the HTTP client the AWS crates are given ──────────────────────────────────────

/// An HTTP client for the AWS crates over `ureq` and the operating system's TLS
/// (spec 103 R9, R11): the SDK's own client carries `hyper` and `rustls`, which
/// this product does not link.
pub(crate) fn ureq_client() -> SharedHttpClient {
    let agent = http_runtime::signed_agent();
    http_client_fn(move |settings, _components| {
        SharedHttpConnector::new(UreqConnector { agent: agent.clone(), timeout: settings_timeout(settings) })
    })
}

fn settings_timeout(settings: &HttpConnectorSettings) -> Duration {
    settings.read_timeout().unwrap_or(DEFAULT_REQUEST_TIMEOUT)
}

#[derive(Debug, Clone)]
struct UreqConnector {
    agent: ureq::Agent,
    timeout: Duration,
}

impl HttpConnector for UreqConnector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let (agent, timeout) = (self.agent.clone(), self.timeout);
        HttpConnectorFuture::new(async move {
            // ureq blocks; the runtime's blocking pool is where blocking belongs.
            tokio::task::spawn_blocking(move || exchange(&agent, request, timeout))
                .await
                .map_err(|e| ConnectorError::other(Box::new(e), None))?
        })
    }
}

/// One request/response through `ureq`.
fn exchange(agent: &ureq::Agent, request: HttpRequest, timeout: Duration) -> Result<HttpResponse, ConnectorError> {
    let method = request.method().to_owned();
    let url = request.uri().to_owned();
    let headers: Vec<(String, String)> = request.headers().iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    let body = request
        .body()
        .bytes()
        .map(<[u8]>::to_vec)
        .ok_or_else(|| ConnectorError::user("a streaming request body is not supported".into()))?;
    let mut reply = http_runtime::send_bytes(agent, &method, &url, &headers, &body, timeout).map_err(|e| match e {
        SendError::Timeout => ConnectorError::timeout("the request timed out".into()),
        SendError::Unreachable(why) => ConnectorError::io(why.into()),
    })?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(&mut reply.body, 16 * 1024 * 1024), &mut bytes)
        .map_err(|e| ConnectorError::io(Box::new(e)))?;
    let status = StatusCode::try_from(reply.status).map_err(|e| ConnectorError::other(Box::new(e), None))?;
    let mut response = HttpResponse::new(status, SdkBody::from(bytes));
    for (name, value) in &reply.headers {
        let _ = response.headers_mut().try_insert(name.clone(), value.clone());
    }
    Ok(response)
}

/// An error and the errors under it, as one line: the SDK's top-level words are
/// generic ("an error occurred while loading credentials"); the cause — an SSO
/// token that is missing, a profile that is not there — is in its sources.
fn describe(e: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![e.to_string()];
    let mut under = e.source();
    while let Some(cause) = under {
        parts.push(cause.to_string());
        under = cause.source();
    }
    one_line(&parts.join(": "))
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    /// A one-shot HTTP server on this machine: it answers one request with
    /// `status` and `body`, and hands back what it was sent.
    fn one_shot(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<(String, Vec<(String, String)>, Vec<u8>)>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/token", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request_line = line.trim().to_owned();
            let mut headers = Vec::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                let l = line.trim_end_matches(['\r', '\n']);
                if l.is_empty() {
                    break;
                }
                let (k, v) = l.split_once(':').unwrap();
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
            }
            let len: usize = headers.iter().find(|(k, _)| k == "content-length").map_or(0, |(_, v)| v.parse().unwrap());
            let mut got = vec![0u8; len];
            reader.read_exact(&mut got).unwrap();
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/x-amz-json-1.1\r\nx-amzn-requestid: abc\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            (request_line, headers, got)
        });
        (url, handle)
    }

    fn request(url: &str, body: &str) -> HttpRequest {
        let mut r = HttpRequest::new(SdkBody::from(body.to_owned()));
        r.set_uri(url.to_owned()).unwrap();
        r.set_method("POST").unwrap();
        r.headers_mut().insert("x-amz-target", "AWSSSOPortalService.GetRoleCredentials");
        r
    }

    /// Spec 103 R9/R11: what the AWS crates send and receive travels through `ureq`
    /// unchanged — method, address, headers and body out; status, headers and body back
    /// — and a refusal (4xx) is an answer, not an error, because the SDK reads it.
    #[test]
    fn the_http_client_carries_a_request_and_its_answer_unchanged() {
        for (status, body) in [(200u16, r#"{"roleCredentials":{"accessKeyId":"EXAMPLE"}}"#), (400, r#"{"__type":"InvalidRequestException"}"#)] {
            let (url, server) = one_shot(status, body);
            let response = exchange(&http_runtime::signed_agent(), request(&url, r#"{"accountId":"1"}"#), Duration::from_secs(5)).unwrap();
            let (request_line, headers, sent) = server.join().unwrap();
            assert_eq!(request_line, "POST /token HTTP/1.1");
            assert_eq!(sent, br#"{"accountId":"1"}"#);
            assert!(headers.iter().any(|(k, v)| k == "x-amz-target" && v == "AWSSSOPortalService.GetRoleCredentials"), "{headers:?}");
            assert_eq!(response.status().as_u16(), status);
            assert_eq!(response.headers().get("x-amzn-requestid"), Some("abc"));
            assert_eq!(response.body().bytes(), Some(body.as_bytes()));
        }
        println!("\n  103 R9 — the AWS crates' HTTP client over ureq: a 200 and a 400 answer carried, request and response unchanged");
    }

    #[test]
    fn an_unreachable_address_is_a_connector_error_not_a_hang() {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let started = std::time::Instant::now();
        let e = exchange(&http_runtime::signed_agent(), request(&format!("http://127.0.0.1:{port}/x"), "{}"), Duration::from_secs(5)).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(5), "{e:?}");
        assert!(e.is_io() || e.is_timeout() || e.is_other(), "{e:?}");
    }
}
