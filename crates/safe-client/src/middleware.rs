//! Middleware factories: Bearer authentication and base-URL rebasing.

use apyx_reqwest_middleware::{HeaderAuthMiddleware, RebaseUrlMiddleware};
use reqwest::{Url, header::HeaderName};

use crate::{Error, network::DEFAULT_BASE_URL};

/// Header carrying the Safe API key.
pub const API_KEY_HEADER: HeaderName = HeaderName::from_static("authorization");

/// Builds the middleware authenticating every request to the Safe API with a
/// `Authorization: Bearer <api_key>` header. The header value is marked
/// sensitive so it is redacted from logs.
pub fn auth_middleware(api_key: &str) -> Result<HeaderAuthMiddleware, Error> {
    HeaderAuthMiddleware::new(API_KEY_HEADER, format!("Bearer {api_key}").as_str())
        .map_err(Error::InvalidApiKey)
}

/// Builds a middleware routing requests built against [`DEFAULT_BASE_URL`] to
/// `base_url` instead — e.g. a non-mainnet network via
/// [`base_url`](crate::base_url), or a mock server in tests.
pub fn rebase_middleware(base_url: Url) -> RebaseUrlMiddleware {
    let default_base_url = DEFAULT_BASE_URL
        .parse()
        .expect("default Safe base URL is valid");

    RebaseUrlMiddleware::new(default_base_url, base_url)
}

#[cfg(test)]
mod tests {
    use reqwest_middleware::ClientBuilder;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;

    #[tokio::test]
    async fn injects_bearer_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ping"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(auth_middleware("test-key").expect("valid key"))
            .build();

        let resp = client
            .get(format!("{}/ping", server.uri()))
            .send()
            .await
            .expect("request");
        assert_eq!(resp.status().as_u16(), 200);
    }
}
