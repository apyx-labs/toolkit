mod client;
mod error;
#[cfg(feature = "reqwest")]
mod middleware;
pub mod model;

pub use client::{DEFAULT_BASE_URL, DuneClient};
pub use error::Error;
#[cfg(feature = "reqwest")]
pub use middleware::{API_KEY_HEADER, auth_middleware, rebase_middleware};
