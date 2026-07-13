//! HTTP client metrics, following the OpenTelemetry HTTP-client semantic
//! conventions and emitted via [`prometheus_client`].
//!
//! Metrics are recorded automatically by
//! [`MetricsMiddleware`](crate::MetricsMiddleware), which
//! sits inside the retry layer and therefore observes every individual HTTP
//! attempt (including retries) with its method, route label, and status code.

use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::Registry,
};

/// Histogram buckets for request duration, in seconds.
const DURATION_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Sentinel `status_code` for attempts that produced no HTTP response
/// (transport errors: timeout, connection refused, decode failure, ...).
pub(crate) const STATUS_TRANSPORT_ERROR: &str = "error";

/// Labels for a completed request attempt.
///
/// `module` identifies which client emitted the series. `method` and `path`
/// are `&'static str` by construction: the method comes from a fixed
/// well-known set and the path from a
/// [`RouteLabel`](crate::RouteLabel) — never from the
/// request URL, which can embed credentials and has unbounded cardinality.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct RequestLabels {
    pub module: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub status_code: String,
}

/// Labels for in-flight requests and retries (no status code yet).
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct AttemptLabels {
    pub module: &'static str,
    pub method: &'static str,
    pub path: &'static str,
}

/// The HTTP client metric families, shared by every client in a service.
///
/// A family may only be registered once per registry (duplicate metric
/// families are invalid exposition), so all HTTP clients record into this one
/// set and tell their series apart by the `module` label: create one
/// `Families`, register it once, and hand each client a [`Metrics`] handle
/// via [`Families::module`].
///
/// Cloning is cheap: each [`Family`] is an `Arc`-backed handle that shares
/// the underlying state, so clones observe into the same series.
#[derive(Clone)]
pub struct Families {
    /// Per-attempt request duration. Its `_count` (sliced by `status_code`)
    /// subsumes a request/error counter; `_sum` and buckets give latency.
    duration: Family<RequestLabels, Histogram>,
    /// In-flight requests.
    active_requests: Family<AttemptLabels, Gauge>,
    /// Retry attempts (attempts after the first for a logical request).
    retries: Family<AttemptLabels, Counter>,
}

impl Default for Families {
    fn default() -> Self {
        Self {
            duration: Family::new_with_constructor(|| {
                Histogram::new(DURATION_BUCKETS.iter().copied())
            }),
            active_requests: Family::default(),
            retries: Family::default(),
        }
    }
}

impl Families {
    /// A recording handle for one client, emitting series labeled
    /// `module=<module>` into these shared families.
    pub fn module(&self, module: &'static str) -> Metrics {
        Metrics {
            module,
            families: self.clone(),
        }
    }

    /// Register all metrics into `registry`. Call once: every [`Metrics`]
    /// handle derived from this set records into the same families. The
    /// counter's `_total` suffix is added by the encoder.
    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "request_duration_seconds",
            "Duration of HTTP client request attempts",
            self.duration.clone(),
        );
        registry.register(
            "active_requests",
            "In-flight HTTP client requests",
            self.active_requests.clone(),
        );
        registry.register(
            "retries",
            "HTTP client retry attempts",
            self.retries.clone(),
        );
    }
}

/// One client's recording handle: [`Families`] plus the `module` label value
/// its series are emitted under.
#[derive(Clone)]
pub struct Metrics {
    /// Value of the `module` label this handle records under, identifying
    /// which client emitted the series.
    module: &'static str,
    pub(crate) families: Families,
}

impl Metrics {
    /// A standalone handle with its own family set, for single-client use
    /// (e.g. a client crate's own tests). Services with several HTTP clients
    /// must share one [`Families`] instead.
    pub fn new(module: &'static str) -> Self {
        Families::default().module(module)
    }

    /// Record a completed attempt's status code and duration.
    pub fn record_request(
        &self,
        method: &'static str,
        path: &'static str,
        status_code: String,
        elapsed_secs: f64,
    ) {
        self.families
            .duration
            .get_or_create(&RequestLabels {
                module: self.module,
                method,
                path,
                status_code,
            })
            .observe(elapsed_secs);
    }

    /// Increment the in-flight gauge as an attempt starts.
    pub fn inc_active(&self, method: &'static str, path: &'static str) {
        self.families
            .active_requests
            .get_or_create(&AttemptLabels {
                module: self.module,
                method,
                path,
            })
            .inc();
    }

    /// Decrement the in-flight gauge as an attempt finishes.
    pub fn dec_active(&self, method: &'static str, path: &'static str) {
        self.families
            .active_requests
            .get_or_create(&AttemptLabels {
                module: self.module,
                method,
                path,
            })
            .dec();
    }

    /// Count a retry (an attempt after the first for a logical request).
    pub fn inc_retry(&self, method: &'static str, path: &'static str) {
        self.families
            .retries
            .get_or_create(&AttemptLabels {
                module: self.module,
                method,
                path,
            })
            .inc();
    }
}
