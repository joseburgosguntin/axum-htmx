//! # Acknowledgements
//! this crate is a modified version of `silkenweb`'s [htmx-axum](<https://github.com/silkenweb/silkenweb/tree/main/packages/htmx-axum>) package but this version does not
//! include silkenweb as dependency as it is inteaded to just be
//! for simple [axum extrator](axum::extract) definitions for `htmx`

use async_trait::async_trait;
use axum::{
    body::{Bytes, HttpBody},
    extract::FromRequest,
    http::{self, header, HeaderMap, Request},
    BoxError,
};
use serde::de::DeserializeOwned;

pub struct HtmxPostRequest<T>(pub T);

#[async_trait]
impl<State, Body, T> FromRequest<State, Body> for HtmxPostRequest<T>
where
    State: Send + Sync,
    Body: HttpBody + Send + 'static,
    Body::Data: Send,
    Body::Error: Into<BoxError>,
    T: DeserializeOwned,
{
    type Rejection = http::StatusCode;

    async fn from_request(req: Request<Body>, state: &State) -> Result<Self, Self::Rejection> {
        if hxmx_content_type(req.headers()) {
            let bytes = Bytes::from_request(req, state)
                .await
                .map_err(|_| http::StatusCode::BAD_REQUEST)?;
            serde_urlencoded::from_bytes(&bytes)
                .map_err(|_| http::StatusCode::BAD_REQUEST)
                .map(HtmxPostRequest)
        } else {
            Err(http::StatusCode::UNSUPPORTED_MEDIA_TYPE)
        }
    }
}

fn hxmx_content_type(headers: &HeaderMap) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return false;
    };

    let is_htmx_content_type = mime.type_() == "application"
        && (mime.subtype() == "x-www-form-urlencoded"
            || mime
                .suffix()
                .map_or(false, |name| name == "x-www-form-urlencoded"));

    is_htmx_content_type
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::post, Router};
    use axum_test_helper::TestClient;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Input {
        foo: String,
    }

    #[tokio::test]
    async fn deserialize_body() {
        let app = Router::new().route(
            "/",
            post(|HtmxPostRequest(input): HtmxPostRequest<Input>| async { input.foo }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/")
            .header("content-type", "application/x-www-form-urlencoded")
            .body("foo=bar")
            .send()
            .await;
        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[tokio::test]
    async fn consume_body_to_htmx_requires_form_urlencoded_content_type() {
        let app = Router::new().route(
            "/",
            post(|input: HtmxPostRequest<Input>| async { input.0.foo }),
        );

        let client = TestClient::new(app);
        let res = client.post("/").body("foo=bar").send().await;

        let status = res.status();

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn form_urlencoded_content_types() {
        async fn valid_form_urlencoded_content_type(content_type: &str) -> bool {
            println!("testing {content_type:?}");

            let app = Router::new().route(
                "/",
                post(
                    |HtmxPostRequest(Input { foo }): HtmxPostRequest<Input>| async {
                        _ = foo;
                    },
                ),
            );

            let res = TestClient::new(app)
                .post("/")
                .header("content-type", content_type)
                .body("foo=bar")
                .send()
                .await;

            res.status() == StatusCode::OK
        }

        assert!(valid_form_urlencoded_content_type("application/x-www-form-urlencoded").await);
        assert!(
            valid_form_urlencoded_content_type("application/x-www-form-urlencoded; charset=utf-8")
                .await
        );
        assert!(
            valid_form_urlencoded_content_type("application/x-www-form-urlencoded;charset=utf-8")
                .await
        );
        assert!(
            valid_form_urlencoded_content_type("application/cloudevents+x-www-form-urlencoded")
                .await
        );
        assert!(!valid_form_urlencoded_content_type("text/x-www-form-urlencoded").await);
    }

    #[tokio::test]
    async fn invalid_form_urlencoded_syntax() {
        let app = Router::new().route(
            "/",
            post(
                |HtmxPostRequest(Input { foo }): HtmxPostRequest<Input>| async {
                    _ = foo;
                },
            ),
        );

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body("fo&&")
            .header("content-type", "application/x-www-form-urlencoded")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
