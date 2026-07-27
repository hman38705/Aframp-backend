//! Traced HTTP client helpers for outbound payment provider calls.
//!
//! Issue #787 — Distributed tracing was not connected to external HTTP calls
//! (Paystack, Flutterwave, Stellar Horizon, etc.). This module provides:
//!
//! - `traced_request` — wraps any `reqwest::RequestBuilder` with an OTel span
//!   that propagates `traceparent`/`tracestate` headers and records outcome.
//! - `TracedClient` — thin newtype around `reqwest::Client` with a convenience
//!   method that applies tracing automatically.
//!
//! # Usage
//!
//! ```rust,no_run
//! use crate::telemetry::traced_client::TracedClient;
//!
//! let client = TracedClient::new();
//! let response = client
//!     .get("https://api.paystack.co/transaction/verify/ref123", "paystack", "/transaction/verify")
//!     .send()
//!     .await?;
//! ```

use reqwest::{Client, RequestBuilder, Response};
use tracing::Instrument;

use crate::telemetry::propagation::inject_context;

/// Execute a `reqwest::RequestBuilder` inside an OpenTelemetry span.
///
/// The span is labelled with `provider` (e.g. `"paystack"`) and `endpoint`
/// (e.g. `"/transaction/verify"`). The `traceparent` / `tracestate` W3C
/// headers are injected into the outbound request so the receiving service can
/// continue the distributed trace.
///
/// Span attributes set:
/// - `otel.kind = "client"`
/// - `http.method`
/// - `peer.service` (provider name)
/// - `http.url` (endpoint path)
/// - `http.status_code` (on response)
/// - `otel.status_code` (`"ERROR"` on HTTP 5xx or network failure)
pub async fn traced_request(
    builder: RequestBuilder,
    provider: &str,
    endpoint: &str,
) -> Result<Response, reqwest::Error> {
    // Clone the builder to inspect the method before consuming it.
    let method = builder
        .try_clone()
        .and_then(|b| b.build().ok())
        .map(|r| r.method().to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());

    let span = tracing::info_span!(
        "outbound_http",
        otel.kind = "client",
        peer.service = %provider,
        http.method = %method,
        http.url = %endpoint,
        http.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
    );

    // Inject trace context into the outbound request headers.
    let builder = {
        let mut headers = reqwest::header::HeaderMap::new();
        inject_context(&mut headers);
        builder.headers(headers)
    };

    async move {
        match builder.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                tracing::Span::current().record("http.status_code", status);
                if response.status().is_server_error() {
                    tracing::Span::current().record("otel.status_code", "ERROR");
                    tracing::warn!(
                        provider = %provider,
                        endpoint = %endpoint,
                        status = status,
                        "outbound request to payment provider returned server error"
                    );
                } else {
                    tracing::Span::current().record("otel.status_code", "OK");
                }
                Ok(response)
            }
            Err(err) => {
                tracing::Span::current().record("otel.status_code", "ERROR");
                tracing::error!(
                    provider = %provider,
                    endpoint = %endpoint,
                    error = %err,
                    "outbound request to payment provider failed"
                );
                Err(err)
            }
        }
    }
    .instrument(span)
    .await
}

/// Thin wrapper around `reqwest::Client` that adds distributed tracing to
/// every outbound request via [`traced_request`].
///
/// Use this client for all calls to external payment providers and
/// Stellar Horizon so traces are end-to-end visible in Jaeger / OTLP backends.
#[derive(Clone, Debug)]
pub struct TracedClient {
    inner: Client,
}

impl TracedClient {
    /// Create a new `TracedClient` with default `reqwest` settings.
    pub fn new() -> Self {
        Self {
            inner: Client::new(),
        }
    }

    /// Create a `TracedClient` wrapping an existing `reqwest::Client`.
    pub fn from_client(client: Client) -> Self {
        Self { inner: client }
    }

    /// Build a traced GET request.
    ///
    /// - `url` — the full request URL.
    /// - `provider` — short identifier for the upstream service (e.g. `"paystack"`).
    /// - `endpoint` — logical endpoint path for the span label (e.g. `"/verify"`).
    pub fn get(&self, url: &str, provider: &str, endpoint: &str) -> TracedRequestBuilder {
        TracedRequestBuilder {
            builder: self.inner.get(url),
            provider: provider.to_string(),
            endpoint: endpoint.to_string(),
        }
    }

    /// Build a traced POST request.
    pub fn post(&self, url: &str, provider: &str, endpoint: &str) -> TracedRequestBuilder {
        TracedRequestBuilder {
            builder: self.inner.post(url),
            provider: provider.to_string(),
            endpoint: endpoint.to_string(),
        }
    }

    /// Build a traced PUT request.
    pub fn put(&self, url: &str, provider: &str, endpoint: &str) -> TracedRequestBuilder {
        TracedRequestBuilder {
            builder: self.inner.put(url),
            provider: provider.to_string(),
            endpoint: endpoint.to_string(),
        }
    }

    /// Expose the underlying `reqwest::Client` for cases where tracing is not
    /// needed (e.g. internal health checks).
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

impl Default for TracedClient {
    fn default() -> Self {
        Self::new()
    }
}

/// A request builder that automatically injects trace context on `.send()`.
pub struct TracedRequestBuilder {
    builder: RequestBuilder,
    provider: String,
    endpoint: String,
}

impl TracedRequestBuilder {
    /// Set a bearer auth token on the request.
    pub fn bearer_auth(self, token: &str) -> Self {
        Self {
            builder: self.builder.bearer_auth(token),
            ..self
        }
    }

    /// Set a JSON body on the request.
    pub fn json<T: serde::Serialize + ?Sized>(self, body: &T) -> Self {
        Self {
            builder: self.builder.json(body),
            ..self
        }
    }

    /// Add a header to the request.
    pub fn header(self, key: &str, value: &str) -> Self {
        Self {
            builder: self.builder.header(key, value),
            ..self
        }
    }

    /// Send the request with distributed tracing applied.
    pub async fn send(self) -> Result<Response, reqwest::Error> {
        traced_request(self.builder, &self.provider, &self.endpoint).await
    }
}
