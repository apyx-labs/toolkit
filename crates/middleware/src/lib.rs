//! HTTP client middleware for [`reqwest_middleware`] clients: static header
//! authentication, base-URL rewriting, and Prometheus metrics following the
//! OpenTelemetry HTTP-client semantic conventions.

mod metrics;
mod middleware;

pub use metrics::{Families, Metrics};
pub use middleware::{
    auth::HeaderAuthMiddleware,
    metrics::{MetricsMiddleware, RouteLabel},
    rebase::RebaseUrlMiddleware,
};
