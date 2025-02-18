// Copyright 2021, 2022 The Matrix.org Foundation C.I.C.
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

use std::time::Duration;

use anyhow::{bail, Context as _};
use hyper::{header::CONTENT_TYPE, Body, Response};
use mas_config::{
    JaegerExporterProtocolConfig, MetricsExporterConfig, Propagator, TelemetryConfig,
    TracingExporterConfig,
};
use opentelemetry::{
    global,
    propagation::TextMapPropagator,
    sdk::{
        self,
        metrics::controllers::BasicController,
        propagation::{BaggagePropagator, TextMapCompositePropagator, TraceContextPropagator},
        trace::{Sampler, Tracer},
        Resource,
    },
    Context,
};
#[cfg(feature = "jaeger")]
use opentelemetry_jaeger::Propagator as JaegerPropagator;
#[cfg(feature = "prometheus")]
use opentelemetry_prometheus::PrometheusExporter;
use opentelemetry_semantic_conventions as semcov;
#[cfg(feature = "zipkin")]
use opentelemetry_zipkin::{B3Encoding, Propagator as ZipkinPropagator};
use tokio::sync::OnceCell;
use url::Url;

static METRICS_BASIC_CONTROLLER: OnceCell<BasicController> = OnceCell::const_new();

#[cfg(feature = "prometheus")]
static PROMETHEUS_EXPORTER: OnceCell<PrometheusExporter> = OnceCell::const_new();

pub async fn setup(
    config: &TelemetryConfig,
) -> anyhow::Result<(Option<Tracer>, Option<BasicController>)> {
    global::set_error_handler(|e| tracing::error!("{}", e))?;
    let propagator = propagator(&config.tracing.propagators)?;

    // The CORS filter needs to know what headers it should whitelist for
    // CORS-protected requests.
    mas_http::set_propagator(&propagator);
    global::set_text_map_propagator(propagator);

    let tracer = tracer(&config.tracing.exporter)
        .await
        .context("Failed to configure traces exporter")?;

    let meter = meter(&config.metrics.exporter).context("Failed to configure metrics exporter")?;
    if let Some(meter) = meter.as_ref() {
        METRICS_BASIC_CONTROLLER.set(meter.clone())?;
    }

    Ok((tracer, meter))
}

pub fn shutdown() {
    global::shutdown_tracer_provider();

    if let Some(controller) = METRICS_BASIC_CONTROLLER.get() {
        let cx = Context::new();
        controller.stop(&cx).unwrap();
    }
}

fn match_propagator(
    propagator: Propagator,
) -> anyhow::Result<Box<dyn TextMapPropagator + Send + Sync>> {
    match propagator {
        Propagator::TraceContext => Ok(Box::new(TraceContextPropagator::new())),
        Propagator::Baggage => Ok(Box::new(BaggagePropagator::new())),

        #[cfg(feature = "jaeger")]
        Propagator::Jaeger => Ok(Box::new(JaegerPropagator::new())),

        #[cfg(feature = "zipkin")]
        Propagator::B3 => Ok(Box::new(ZipkinPropagator::with_encoding(
            B3Encoding::SingleHeader,
        ))),

        #[cfg(feature = "zipkin")]
        Propagator::B3Multi => Ok(Box::new(ZipkinPropagator::with_encoding(
            B3Encoding::MultipleHeader,
        ))),

        p => bail!(
            "The service was compiled without support for the {p:?} propagator, but config uses it.",
        ),
    }
}

fn propagator(propagators: &[Propagator]) -> anyhow::Result<impl TextMapPropagator> {
    let propagators: Result<Vec<_>, _> =
        propagators.iter().cloned().map(match_propagator).collect();

    Ok(TextMapCompositePropagator::new(propagators?))
}

#[cfg(any(feature = "zipkin", feature = "jaeger"))]
async fn http_client() -> anyhow::Result<impl opentelemetry_http::HttpClient + 'static> {
    let client = mas_http::make_untraced_client()
        .await
        .context("Failed to build HTTP client used by telemetry exporter")?;
    let client =
        opentelemetry_http::hyper::HyperClient::new_with_timeout(client, Duration::from_secs(30));
    Ok(client)
}

fn stdout_tracer() -> Tracer {
    sdk::export::trace::stdout::new_pipeline()
        .with_pretty_print(true)
        .with_trace_config(trace_config())
        .install_simple()
}

#[cfg(feature = "otlp")]
fn otlp_tracer(endpoint: &Option<Url>) -> anyhow::Result<Tracer> {
    use opentelemetry_otlp::WithExportConfig;

    let mut exporter = opentelemetry_otlp::new_exporter().tonic();
    if let Some(endpoint) = endpoint {
        exporter = exporter.with_endpoint(endpoint.to_string());
    }

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(trace_config())
        .install_batch(opentelemetry::runtime::Tokio)
        .context("Failed to configure OTLP trace exporter")?;

    Ok(tracer)
}

#[cfg(not(feature = "otlp"))]
fn otlp_tracer(endpoint: &Option<Url>) -> anyhow::Result<Tracer> {
    let _ = endpoint;
    anyhow::bail!("The service was compiled without OTLP exporter support, but config exports traces via OTLP.")
}

#[cfg(not(feature = "jaeger"))]
fn jaeger_agent_tracer(host: &str, port: u16) -> anyhow::Result<Tracer> {
    let _ = host;
    let _ = port;
    anyhow::bail!("The service was compiled without Jaeger exporter support, but config exports traces via Jaeger.")
}

#[cfg(feature = "jaeger")]
fn jaeger_agent_tracer(host: &str, port: u16) -> anyhow::Result<Tracer> {
    let pipeline = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_trace_config(trace_config())
        .with_endpoint((host, port));

    let tracer = pipeline
        .install_batch(opentelemetry::runtime::Tokio)
        .context("Failed to configure Jaeger agent exporter")?;

    Ok(tracer)
}

#[cfg(not(feature = "jaeger"))]
async fn jaeger_collector_tracer(
    endpoint: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> anyhow::Result<Tracer> {
    let _ = endpoint;
    let _ = username;
    let _ = password;
    futures_util::future::ready(()).await; // Silence the "unused async" lint
    anyhow::bail!("The service was compiled without Jaeger exporter support, but config exports traces via Jaeger.")
}

#[cfg(feature = "jaeger")]
async fn jaeger_collector_tracer(
    endpoint: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> anyhow::Result<Tracer> {
    let http_client = http_client().await?;
    let mut pipeline = opentelemetry_jaeger::new_collector_pipeline()
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_trace_config(trace_config())
        .with_http_client(http_client)
        .with_endpoint(endpoint);

    if let Some(username) = username {
        pipeline = pipeline.with_username(username);
    }

    if let Some(password) = password {
        pipeline = pipeline.with_password(password);
    }

    let tracer = pipeline
        .install_batch(opentelemetry::runtime::Tokio)
        .context("Failed to configure Jaeger collector exporter")?;

    Ok(tracer)
}

#[cfg(not(feature = "zipkin"))]
async fn zipkin_tracer(collector_endpoint: &Option<Url>) -> anyhow::Result<Tracer> {
    let _ = collector_endpoint;
    futures_util::future::ready(()).await; // Silence the "unused async" lint
    anyhow::bail!("The service was compiled without Jaeger exporter support, but config exports traces via Jaeger.")
}

#[cfg(feature = "zipkin")]
async fn zipkin_tracer(collector_endpoint: &Option<Url>) -> anyhow::Result<Tracer> {
    let http_client = http_client().await?;

    let mut pipeline = opentelemetry_zipkin::new_pipeline()
        .with_http_client(http_client)
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_trace_config(trace_config());

    if let Some(collector_endpoint) = collector_endpoint {
        pipeline = pipeline.with_collector_endpoint(collector_endpoint.as_str());
    }

    let tracer = pipeline
        .install_batch(opentelemetry::runtime::Tokio)
        .context("Failed to configure Zipkin exporter")?;

    Ok(tracer)
}

async fn tracer(config: &TracingExporterConfig) -> anyhow::Result<Option<Tracer>> {
    let tracer = match config {
        TracingExporterConfig::None => return Ok(None),
        TracingExporterConfig::Stdout => stdout_tracer(),
        TracingExporterConfig::Otlp { endpoint } => otlp_tracer(endpoint)?,
        TracingExporterConfig::Jaeger(JaegerExporterProtocolConfig::UdpThriftCompact {
            agent_host,
            agent_port,
        }) => jaeger_agent_tracer(agent_host, *agent_port)?,
        TracingExporterConfig::Jaeger(JaegerExporterProtocolConfig::HttpThriftBinary {
            endpoint,
            username,
            password,
        }) => jaeger_collector_tracer(endpoint, username.as_deref(), password.as_deref()).await?,
        TracingExporterConfig::Zipkin { collector_endpoint } => {
            zipkin_tracer(collector_endpoint).await?
        }
    };

    Ok(Some(tracer))
}

#[cfg(feature = "otlp")]
fn otlp_meter(endpoint: &Option<url::Url>) -> anyhow::Result<BasicController> {
    use opentelemetry_otlp::WithExportConfig;

    let mut exporter = opentelemetry_otlp::new_exporter().tonic();
    if let Some(endpoint) = endpoint {
        exporter = exporter.with_endpoint(endpoint.to_string());
    }

    let controller = opentelemetry_otlp::new_pipeline()
        .metrics(
            sdk::metrics::selectors::simple::inexpensive(),
            sdk::export::metrics::aggregation::cumulative_temporality_selector(),
            opentelemetry::runtime::Tokio,
        )
        .with_resource(resource())
        .with_exporter(exporter)
        .build()
        .context("Failed to configure OTLP metrics exporter")?;

    Ok(controller)
}

#[cfg(not(feature = "otlp"))]
fn otlp_meter(endpoint: &Option<url::Url>) -> anyhow::Result<BasicController> {
    let _ = endpoint;
    anyhow::bail!("The service was compiled without OTLP exporter support, but config exports metrics via OTLP.")
}

fn stdout_meter() -> anyhow::Result<BasicController> {
    let exporter = sdk::export::metrics::stdout().build()?;
    let controller = sdk::metrics::controllers::basic(sdk::metrics::processors::factory(
        sdk::metrics::selectors::simple::inexpensive(),
        exporter.temporality_selector(),
    ))
    .with_resource(resource())
    .with_exporter(exporter)
    .build();

    let cx = Context::new();
    controller.start(&cx, opentelemetry::runtime::Tokio)?;

    global::set_meter_provider(controller.clone());
    Ok(controller)
}

#[cfg(not(feature = "prometheus"))]
pub fn prometheus_service<T>() -> tower::util::ServiceFn<
    impl FnMut(T) -> std::future::Ready<Result<Response<Body>, std::convert::Infallible>> + Clone,
> {
    tracing::warn!("Prometheus exporter was not enabled at compilation time, but the Prometheus resource was mounted on a listener");

    tower::service_fn(move |_req| {
        let response = Response::builder()
            .status(500)
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::from(
                "Prometheus exporter was not enabled at compilation time",
            ))
            .unwrap();

        std::future::ready(Ok(response))
    })
}

#[cfg(feature = "prometheus")]
pub fn prometheus_service<T>() -> tower::util::ServiceFn<
    impl FnMut(T) -> std::future::Ready<Result<Response<Body>, std::convert::Infallible>> + Clone,
> {
    use prometheus::{Encoder, TextEncoder};

    if !PROMETHEUS_EXPORTER.initialized() {
        tracing::warn!("A Prometheus resource was mounted on a listener, but the Prometheus exporter was not setup in the config");
    }

    tower::service_fn(move |_req| {
        let response = if let Some(exporter) = PROMETHEUS_EXPORTER.get() {
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = exporter.registry().gather();

            // That shouldn't panic, unless we're constructing invalid labels
            encoder.encode(&metric_families, &mut buffer).unwrap();

            Response::builder()
                .status(200)
                .header(CONTENT_TYPE, encoder.format_type())
                .body(Body::from(buffer))
                .unwrap()
        } else {
            Response::builder()
                .status(500)
                .header(CONTENT_TYPE, "text/plain")
                .body(Body::from("Prometheus exporter was not enabled in config"))
                .unwrap()
        };

        std::future::ready(Ok(response))
    })
}

#[cfg(not(feature = "prometheus"))]
fn prometheus_meter() -> anyhow::Result<BasicController> {
    anyhow::bail!("The service was compiled without Prometheus exporter support, but config exports metrics via Prometheus.")
}

#[cfg(feature = "prometheus")]
fn prometheus_meter() -> anyhow::Result<BasicController> {
    let controller = sdk::metrics::controllers::basic(
        sdk::metrics::processors::factory(
            // All histogram metrics are in milliseconds. Each bucket is ~2x the previous one.
            sdk::metrics::selectors::simple::histogram([
                1.0, 3.0, 5.0, 10.0, 30.0, 50.0, 100.0, 300.0, 1000.0,
            ]),
            sdk::export::metrics::aggregation::cumulative_temporality_selector(),
        )
        .with_memory(true),
    )
    .with_resource(resource())
    .build();

    let exporter = opentelemetry_prometheus::exporter(controller.clone()).try_init()?;
    PROMETHEUS_EXPORTER.set(exporter)?;

    Ok(controller)
}

fn meter(config: &MetricsExporterConfig) -> anyhow::Result<Option<BasicController>> {
    let controller = match config {
        MetricsExporterConfig::None => None,
        MetricsExporterConfig::Stdout => Some(stdout_meter()?),
        MetricsExporterConfig::Otlp { endpoint } => Some(otlp_meter(endpoint)?),
        MetricsExporterConfig::Prometheus => Some(prometheus_meter()?),
    };

    Ok(controller)
}

fn trace_config() -> sdk::trace::Config {
    sdk::trace::config()
        .with_resource(resource())
        .with_sampler(Sampler::AlwaysOn)
}

fn resource() -> Resource {
    let resource = Resource::new(vec![
        semcov::resource::SERVICE_NAME.string(env!("CARGO_PKG_NAME")),
        semcov::resource::SERVICE_VERSION.string(env!("CARGO_PKG_VERSION")),
    ]);

    let detected = Resource::from_detectors(
        Duration::from_secs(5),
        vec![
            Box::new(sdk::resource::EnvResourceDetector::new()),
            Box::new(sdk::resource::OsResourceDetector),
            Box::new(sdk::resource::ProcessResourceDetector),
        ],
    );

    resource.merge(&detected)
}
