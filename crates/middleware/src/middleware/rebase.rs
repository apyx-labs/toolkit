use http::Extensions;
use reqwest::{Request, Response, Url};
use reqwest_middleware::{Middleware, Next, Result};

/// Rewrites requests targeting one base URL onto another.
///
/// Lets client code build requests against a canonical API base URL while the
/// effective destination stays a construction-time concern — e.g. pointing a
/// client at a staging environment or a mock server in tests — without
/// threading configuration through every call site.
///
/// Requests whose URL does not start with `from` pass through untouched.
pub struct RebaseUrlMiddleware {
    from: Url,
    to: Url,
}

impl RebaseUrlMiddleware {
    /// `from` and `to` are treated as URL prefixes; the part of the request
    /// URL following the `from` prefix is preserved, as is the query string.
    pub fn new(from: Url, to: Url) -> Self {
        Self { from, to }
    }

    fn rebase(&self, url: &Url) -> Option<Url> {
        if url.scheme() != self.from.scheme() || url.authority() != self.from.authority() {
            return None;
        }
        let suffix = url.path().strip_prefix(self.from.path())?;

        let mut rebased = self.to.clone();
        rebased.set_path(&format!("{}{suffix}", self.to.path().trim_end_matches('/')));
        rebased.set_query(url.query());
        Some(rebased)
    }
}

#[async_trait::async_trait]
impl Middleware for RebaseUrlMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        ext: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        if let Some(rebased) = self.rebase(req.url()) {
            *req.url_mut() = rebased;
        }

        next.run(req, ext).await
    }
}

#[cfg(test)]
mod tests {
    use reqwest_middleware::ClientBuilder;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    use super::*;

    fn rebase(from: &str, to: &str, url: &str) -> Option<String> {
        let middleware = RebaseUrlMiddleware::new(
            from.parse().expect("valid from url"),
            to.parse().expect("valid to url"),
        );
        middleware
            .rebase(&url.parse().expect("valid request url"))
            .map(String::from)
    }

    #[test]
    fn rebases_matching_prefix_preserving_suffix_and_query() {
        assert_eq!(
            rebase(
                "https://api.example.com/api/v1",
                "http://localhost:8080/mock",
                "https://api.example.com/api/v1/things/42?limit=10",
            )
            .as_deref(),
            Some("http://localhost:8080/mock/things/42?limit=10"),
        );
    }

    #[test]
    fn passes_through_non_matching_urls() {
        assert_eq!(
            rebase(
                "https://api.example.com/api/v1",
                "http://localhost:8080",
                "https://other.example.com/api/v1/things",
            ),
            None,
        );
        assert_eq!(
            rebase(
                "https://api.example.com/api/v1",
                "http://localhost:8080",
                "https://api.example.com/api/v2/things",
            ),
            None,
        );
    }

    #[tokio::test]
    async fn rewrites_request_destination() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/things/42"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RebaseUrlMiddleware::new(
                "https://api.example.com/api/v1".parse().expect("valid url"),
                server.uri().parse().expect("valid url"),
            ))
            .build();

        let response = client
            .get("https://api.example.com/api/v1/things/42?limit=10")
            .send()
            .await
            .expect("request succeeds");
        assert_eq!(response.status().as_u16(), 200);
    }
}
