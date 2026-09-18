//! A small custom middleware -- Module 5's `actix_web::middleware::from_fn`
//! lesson (`examples::module_5::custom_middleware`), applied for real.
//!
//! Adds an `X-Response-Time-Ms` header to every response. See `main.rs` for
//! where this sits relative to CORS and `TracingLogger`: registered so it
//! wraps CORS (but is itself wrapped by `TracingLogger`), which means even
//! a CORS-short-circuited preflight response still gets timed, and every
//! timed response still gets logged.

use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header::{HeaderName, HeaderValue},
    middleware::Next,
    Error,
};
use std::time::Instant;

pub async fn timing(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let start = Instant::now();

    // Everything downstream -- any middleware registered inside this one,
    // then the handler -- runs inside this `.await`.
    let mut res = next.call(req).await?;

    let elapsed_ms = start.elapsed().as_millis();
    res.headers_mut().insert(
        HeaderName::from_static("x-response-time-ms"),
        HeaderValue::from_str(&elapsed_ms.to_string())
            .expect("a millisecond count always formats as valid header text"),
    );

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{middleware::from_fn, test, web, App, HttpResponse};

    #[actix_web::test]
    async fn adds_a_response_time_header_to_a_normal_response() {
        let app = test::init_service(
            App::new()
                .wrap(from_fn(timing))
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;

        let header = res
            .headers()
            .get("x-response-time-ms")
            .expect("timing middleware should set the header")
            .to_str()
            .expect("header value should be ASCII");
        assert!(
            header.parse::<u128>().is_ok(),
            "header value should be a plain number, got {header:?}"
        );
    }
}
