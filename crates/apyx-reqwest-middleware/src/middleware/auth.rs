use http::Extensions;
use reqwest::{
    Request, Response,
    header::{HeaderName, HeaderValue, InvalidHeaderValue},
};
use reqwest_middleware::{Middleware, Next, Result};

/// Injects a static authentication header into every request.
///
/// The header value is always marked sensitive so it is redacted from `Debug`
/// output and excluded from logs.
pub struct HeaderAuthMiddleware {
    name: HeaderName,
    value: HeaderValue,
}

impl HeaderAuthMiddleware {
    pub fn new(name: HeaderName, value: &str) -> std::result::Result<Self, InvalidHeaderValue> {
        let mut value = HeaderValue::from_str(value)?;
        value.set_sensitive(true);

        Ok(Self { name, value })
    }
}

#[async_trait::async_trait]
impl Middleware for HeaderAuthMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        ext: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        req.headers_mut()
            .insert(self.name.clone(), self.value.clone());

        next.run(req, ext).await
    }
}

#[cfg(test)]
mod tests {
    use reqwest_middleware::ClientBuilder;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;

    #[test]
    fn header_value_is_sensitive() {
        let middleware = HeaderAuthMiddleware::new(HeaderName::from_static("x-api-key"), "secret")
            .expect("valid header value");

        assert!(middleware.value.is_sensitive());
    }

    #[tokio::test]
    async fn injects_header_on_every_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .and(header("x-api-key", "secret"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(
                HeaderAuthMiddleware::new(HeaderName::from_static("x-api-key"), "secret")
                    .expect("valid header value"),
            )
            .build();

        let response = client
            .get(format!("{}/thing", server.uri()))
            .send()
            .await
            .expect("request succeeds");
        assert_eq!(response.status().as_u16(), 200);
    }
}
