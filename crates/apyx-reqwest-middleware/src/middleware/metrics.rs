use std::time::Instant;

use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result};

use crate::metrics::{Metrics, STATUS_TRANSPORT_ERROR};

/// Low-cardinality route label for HTTP client metrics.
///
/// Insert into a request's middleware extensions via
/// [`reqwest_middleware::RequestBuilder::with_extension`]. Requests without
/// one are labeled `path="unknown"` — the middleware never uses the raw URL
/// path, which can embed credentials (e.g. `/v2/<api-key>/...`) and has
/// unbounded cardinality.
#[derive(Clone, Copy, Debug)]
pub struct RouteLabel(pub &'static str);

/// Fallback `path` label for requests carrying no [`RouteLabel`].
const UNKNOWN_ROUTE: &str = "unknown";

/// Per-request attempt counter, stored in the request [`Extensions`] so it
/// survives across retries (`reqwest-retry` threads the same `Extensions`
/// through every attempt). Used to distinguish retries from the first attempt.
/// Assumes a fresh `Extensions` per logical request: a reused one would
/// miscount the next request's first attempt as a retry.
#[derive(Clone, Copy)]
struct AttemptCount(u32);

/// `method` label per the OTel `http.request.method` convention: the
/// well-known set verbatim, anything else collapsed to `_OTHER`.
fn method_label(method: &reqwest::Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        "CONNECT" => "CONNECT",
        "OPTIONS" => "OPTIONS",
        "TRACE" => "TRACE",
        "PATCH" => "PATCH",
        _ => "_OTHER",
    }
}

/// Records HTTP client metrics for every individual request attempt.
///
/// Registered *inside* the retry middleware so it observes each attempt — its
/// real method, route label, and status (or transport error) — which is what
/// lets it capture retries and intermediate status codes.
pub struct MetricsMiddleware {
    metrics: Metrics,
}

impl MetricsMiddleware {
    pub fn new(metrics: Metrics) -> Self {
        Self { metrics }
    }
}

#[async_trait::async_trait]
impl Middleware for MetricsMiddleware {
    async fn handle(&self, req: Request, ext: &mut Extensions, next: Next<'_>) -> Result<Response> {
        let method = method_label(req.method());
        let path = ext.get::<RouteLabel>().map_or(UNKNOWN_ROUTE, |r| r.0);

        // Any attempt beyond the first is a retry.
        let attempt = ext.get::<AttemptCount>().map_or(0, |c| c.0);
        if attempt > 0 {
            self.metrics.inc_retry(method, path);
        }
        ext.insert(AttemptCount(attempt + 1));

        // The decrement has to survive the future being dropped at the await
        // below: hyper cancels a handler when its downstream client
        // disconnects, and a caller-side `timeout` cancels the same way. A
        // plain statement after `.await` never runs on that path, so every
        // abandoned request permanently leaks one count — and a gauge only
        // falls back to truth on process restart.
        let _active = ActiveRequest::begin(&self.metrics, method, path);
        let start = Instant::now();
        let result = next.run(req, ext).await;
        let elapsed = start.elapsed().as_secs_f64();

        let status_code = match &result {
            Ok(response) => response.status().as_u16().to_string(),
            Err(_) => STATUS_TRANSPORT_ERROR.to_owned(),
        };
        self.metrics
            .record_request(method, path, status_code, elapsed);

        result
    }
}

/// Holds one count on the in-flight gauge for as long as it is alive.
///
/// `Drop` rather than a trailing `dec_active` call so cancellation releases
/// the count too — see [`MetricsMiddleware::handle`]. Labels are `&'static
/// str`, so the guard borrows nothing from the request.
struct ActiveRequest {
    metrics: Metrics,
    method: &'static str,
    path: &'static str,
}

impl ActiveRequest {
    /// Increment the gauge. Cloning [`Metrics`] is cheap — it is an
    /// `Arc`-backed handle onto the same families.
    fn begin(metrics: &Metrics, method: &'static str, path: &'static str) -> Self {
        metrics.inc_active(method, path);
        Self {
            metrics: metrics.clone(),
            method,
            path,
        }
    }
}

impl Drop for ActiveRequest {
    fn drop(&mut self) {
        self.metrics.dec_active(self.method, self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use prometheus_client::{encoding::text::encode, registry::Registry};
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
    use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    fn metered_client(metrics: Metrics) -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new())
            .with(MetricsMiddleware::new(metrics))
            .build()
    }

    fn encode_metrics(metrics: &Metrics) -> String {
        let mut registry = Registry::default();
        metrics.families.register(&mut registry);
        let mut encoded = String::new();
        encode(&mut encoded, &registry).expect("encode");
        encoded
    }

    /// The whole point of `RouteLabel`: a URL with a credential embedded in
    /// its path (Alchemy-style) must never reach the metrics exposition. The
    /// label comes from the extension; the secret appears nowhere.
    #[tokio::test]
    async fn route_label_masks_secret_url_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/sekret123/endpoint"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let metrics = Metrics::new("test-client");
        let client = metered_client(metrics.clone());

        client
            .get(format!("{}/v2/sekret123/endpoint", server.uri()))
            .with_extension(RouteLabel("/v2/{key}/endpoint"))
            .send()
            .await
            .expect("request succeeds");

        let encoded = encode_metrics(&metrics);
        assert!(
            encoded.contains(r#"path="/v2/{key}/endpoint""#),
            "expected the route label as path:\n{encoded}"
        );
        assert!(
            !encoded.contains("sekret123"),
            "secret must never appear in metrics:\n{encoded}"
        );
    }

    /// Requests without a `RouteLabel` degrade to `path="unknown"` — never
    /// the raw URL path.
    #[tokio::test]
    async fn missing_route_label_falls_back_to_unknown() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let metrics = Metrics::new("test-client");
        let client = metered_client(metrics.clone());

        client
            .get(format!("{}/thing", server.uri()))
            .send()
            .await
            .expect("request succeeds");

        let encoded = encode_metrics(&metrics);
        assert!(
            encoded.contains(r#"path="unknown""#),
            "expected unknown fallback:\n{encoded}"
        );
        assert!(
            !encoded.contains(r#"path="/thing""#),
            "raw URL path must never be a label:\n{encoded}"
        );
    }

    /// A server that always returns 503 drives `reqwest-retry` to exhaust its
    /// retries. The middleware sits inside the retry loop, so it should observe
    /// every attempt, count the retries, and leave the in-flight gauge at zero.
    /// `RouteLabel` lives in the `Extensions` that the retry middleware threads
    /// through every attempt, so the label must hold across all of them.
    #[tokio::test]
    async fn records_each_attempt_status_and_retries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        // 2 retries with tiny backoff bounds keeps the test fast => 3 attempts.
        let policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(1), Duration::from_millis(5))
            .build_with_max_retries(2);
        let metrics = Metrics::new("test-client");
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(policy))
            .with(MetricsMiddleware::new(metrics.clone()))
            .build();

        let response = client
            .get(format!("{}/thing", server.uri()))
            .with_extension(RouteLabel("/thing"))
            .send()
            .await
            .expect("request completes after retries");
        assert_eq!(response.status().as_u16(), 503);

        let encoded = encode_metrics(&metrics);
        // 3 attempts total, all 503.
        assert!(
            encoded.contains(
                r#"request_duration_seconds_count{module="test-client",method="GET",path="/thing",status_code="503"} 3"#
            ),
            "expected 3 recorded 503 attempts:\n{encoded}"
        );
        // 2 of those attempts were retries.
        assert!(
            encoded.contains(r#"retries_total{module="test-client",method="GET",path="/thing"} 2"#),
            "expected 2 retries:\n{encoded}"
        );
        // In-flight gauge returns to zero once the request finishes.
        assert!(
            encoded
                .contains(r#"active_requests{module="test-client",method="GET",path="/thing"} 0"#),
            "expected in-flight gauge back to 0:\n{encoded}"
        );
    }

    /// A caller that gives up mid-flight (here: `tokio::time::timeout`, in
    /// production: hyper dropping a proxy handler when the downstream client
    /// disconnects) drops the middleware future at its `next.run` await. The
    /// in-flight gauge must still return to zero — a decrement that only runs
    /// on the completion path leaks one count per abandoned request, and a
    /// gauge only ever climbs back down on process restart. Observed in the
    /// egress proxy as `active_requests` ramping to 1159 during an upstream
    /// brownout and never draining.
    #[tokio::test]
    async fn a_cancelled_request_still_releases_the_in_flight_gauge() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&server)
            .await;

        let metrics = Metrics::new("test-client");
        let client = metered_client(metrics.clone());

        let elapsed = tokio::time::timeout(
            Duration::from_millis(50),
            client
                .get(format!("{}/slow", server.uri()))
                .with_extension(RouteLabel("/slow"))
                .send(),
        )
        .await;
        assert!(
            elapsed.is_err(),
            "the request must still be in flight when dropped"
        );

        let encoded = encode_metrics(&metrics);
        assert!(
            encoded
                .contains(r#"active_requests{module="test-client",method="GET",path="/slow"} 0"#),
            "cancelled request leaked an in-flight count:\n{encoded}"
        );
    }
}
