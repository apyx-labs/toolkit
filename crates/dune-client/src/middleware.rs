use apyx_reqwest_middleware::{HeaderAuthMiddleware, RebaseUrlMiddleware};
use reqwest::{Url, header::HeaderName};

use crate::{Error, client::DEFAULT_BASE_URL};

/// Header carrying the Dune API key.
pub const API_KEY_HEADER: HeaderName = HeaderName::from_static("x-dune-api-key");

/// Builds the middleware authenticating every request to the Dune API.
pub fn auth_middleware(api_key: &str) -> Result<HeaderAuthMiddleware, Error> {
    HeaderAuthMiddleware::new(API_KEY_HEADER, api_key).map_err(Error::InvalidApiKey)
}

/// Builds a middleware routing requests to `base_url` instead of the default
/// Dune base URL — e.g. a proxy or a mock server in tests.
pub fn rebase_middleware(base_url: Url) -> RebaseUrlMiddleware {
    let default_base_url = DEFAULT_BASE_URL
        .parse()
        .expect("default Dune base URL is valid");

    RebaseUrlMiddleware::new(default_base_url, base_url)
}
