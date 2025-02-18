// Copyright 2022 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{sync::Arc, time::Duration};

use headers::{ContentLength, HeaderMapExt, Host, UserAgent};
use http::{header::USER_AGENT, HeaderValue, Request, Response};
use hyper::client::connect::HttpInfo;
use mas_tower::{
    EnrichSpan, MakeSpan, TraceContextLayer, TraceContextService, TraceLayer, TraceService,
};
use tokio::sync::Semaphore;
use tower::{
    limit::{ConcurrencyLimit, GlobalConcurrencyLimitLayer},
    Layer,
};
use tower_http::{
    follow_redirect::{FollowRedirect, FollowRedirectLayer},
    set_header::{SetRequestHeader, SetRequestHeaderLayer},
    timeout::{Timeout, TimeoutLayer},
};
use tracing::Span;

pub type ClientService<S> = SetRequestHeader<
    ConcurrencyLimit<
        FollowRedirect<
            TraceService<
                TraceContextService<Timeout<S>>,
                MakeSpanForRequest,
                EnrichSpanOnResponse,
                EnrichSpanOnError,
            >,
        >,
    >,
    HeaderValue,
>;

#[derive(Debug, Clone)]
pub struct MakeSpanForRequest;

impl<B> MakeSpan<Request<B>> for MakeSpanForRequest {
    fn make_span(&self, request: &Request<B>) -> Span {
        let headers = request.headers();
        let host = headers.typed_get::<Host>().map(tracing::field::display);
        let user_agent = headers
            .typed_get::<UserAgent>()
            .map(tracing::field::display);
        let content_length = headers.typed_get().map(|ContentLength(len)| len);
        let net_sock_peer_name = request.uri().host();

        tracing::info_span!(
            "http.client.request",
            "otel.kind" = "client",
            "otel.status_code" = tracing::field::Empty,
            "http.method" = %request.method(),
            "http.url" = %request.uri(),
            "http.status_code" = tracing::field::Empty,
            "http.host" = host,
            "http.request_content_length" = content_length,
            "http.response_content_length" = tracing::field::Empty,
            "net.transport" = "ip_tcp",
            "net.sock.family" = tracing::field::Empty,
            "net.sock.peer.name" = net_sock_peer_name,
            "net.sock.peer.addr" = tracing::field::Empty,
            "net.sock.peer.port" = tracing::field::Empty,
            "net.sock.host.addr" = tracing::field::Empty,
            "net.sock.host.port" = tracing::field::Empty,
            "user_agent.original" = user_agent,
            "rust.error" = tracing::field::Empty,
        )
    }
}

#[derive(Debug, Clone)]
pub struct EnrichSpanOnResponse;

impl<B> EnrichSpan<Response<B>> for EnrichSpanOnResponse {
    fn enrich_span(&self, span: &Span, response: &Response<B>) {
        span.record("otel.status_code", "OK");
        span.record("http.status_code", response.status().as_u16());

        if let Some(ContentLength(content_length)) = response.headers().typed_get() {
            span.record("http.response_content_length", content_length);
        }

        if let Some(http_info) = response.extensions().get::<HttpInfo>() {
            let local = http_info.local_addr();
            let remote = http_info.remote_addr();

            let family = if local.is_ipv4() { "inet" } else { "inet6" };
            span.record("net.sock.family", family);
            span.record("net.sock.peer.addr", remote.ip().to_string());
            span.record("net.sock.peer.port", remote.port());
            span.record("net.sock.host.addr", local.ip().to_string());
            span.record("net.sock.host.port", local.port());
        } else {
            tracing::warn!("No HttpInfo injected in response extensions");
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnrichSpanOnError;

impl<E> EnrichSpan<E> for EnrichSpanOnError
where
    E: std::error::Error + 'static,
{
    fn enrich_span(&self, span: &Span, error: &E) {
        span.record("otel.status_code", "ERROR");
        span.record("rust.error", error as &dyn std::error::Error);
    }
}

#[derive(Debug, Clone)]
pub struct ClientLayer {
    user_agent_layer: SetRequestHeaderLayer<HeaderValue>,
    concurrency_limit_layer: GlobalConcurrencyLimitLayer,
    follow_redirect_layer: FollowRedirectLayer,
    trace_layer: TraceLayer<MakeSpanForRequest, EnrichSpanOnResponse, EnrichSpanOnError>,
    trace_context_layer: TraceContextLayer,
    timeout_layer: TimeoutLayer,
}

impl Default for ClientLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientLayer {
    #[must_use]
    pub fn new() -> Self {
        let semaphore = Arc::new(Semaphore::new(10));
        Self::with_semaphore(semaphore)
    }

    #[must_use]
    pub fn with_semaphore(semaphore: Arc<Semaphore>) -> Self {
        Self {
            user_agent_layer: SetRequestHeaderLayer::overriding(
                USER_AGENT,
                HeaderValue::from_static("matrix-authentication-service/0.0.1"),
            ),
            concurrency_limit_layer: GlobalConcurrencyLimitLayer::with_semaphore(semaphore),
            follow_redirect_layer: FollowRedirectLayer::new(),
            trace_layer: TraceLayer::new(MakeSpanForRequest)
                .on_response(EnrichSpanOnResponse)
                .on_error(EnrichSpanOnError),
            trace_context_layer: TraceContextLayer::new(),
            timeout_layer: TimeoutLayer::new(Duration::from_secs(10)),
        }
    }
}

impl<S> Layer<S> for ClientLayer
where
    S: Clone,
{
    type Service = ClientService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        (
            &self.user_agent_layer,
            &self.concurrency_limit_layer,
            &self.follow_redirect_layer,
            &self.trace_layer,
            &self.trace_context_layer,
            &self.timeout_layer,
        )
            .layer(inner)
    }
}
