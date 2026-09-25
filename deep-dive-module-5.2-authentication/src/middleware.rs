//! Custom middleware built with `actix_web::middleware::from_fn` -- Module
//! 5's lesson (`examples::module_5::custom_middleware`), applied for real
//! twice over: [`timing`] adds a response-timing header, and [`require_auth`]
//! (Module 6) protects a route scope with a JWT bearer check. Deliberately
//! the *same* primitive for both -- see this crate's Module 6 README on why
//! auth middleware doesn't reach for a second crate/ceremony on top of
//! `from_fn`.
//!
//! See `main.rs` for where `timing` sits relative to CORS and
//! `TracingLogger`: registered so it wraps CORS (but is itself wrapped by
//! `TracingLogger`), which means even a CORS-short-circuited preflight
//! response still gets timed, and every timed response still gets logged.
//! See `routes::tasks::configure` for where `require_auth` is wrapped
//! around the `/tasks` scope specifically, and not the whole app.

use crate::{auth::Claims, config::AppConfig, error::AppError};
use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header::{self, HeaderName, HeaderValue},
    middleware::Next,
    web, Error, HttpMessage, ResponseError,
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

/// Validates a bearer token before letting a request reach anything wrapped
/// by this middleware. Module 6's applied-for-real lesson
/// (`examples::module_6::jwt_middleware`) -- see this crate's README for why
/// a missing header and a bad token both collapse into the same
/// `AppError::Unauthorized`, and why the claims are only ever stashed in
/// `req.extensions_mut()`, never echoed back in a response.
///
/// Builds the `401` response directly on the rejection path, rather than
/// the simpler-looking `extract_claims(..).map_err(|_| AppError::
/// Unauthorized)?`: a `from_fn` middleware that short-circuits by
/// returning `Err` from `Service::call` only gets converted into a real
/// HTTP response by the full `HttpServer` dispatcher `main.rs` runs --
/// `actix_web::test::call_service` (used throughout this crate's own
/// tests) calls the built service directly and panics on a bare `Err`
/// instead of turning it into a response. Building the `ServiceResponse`
/// here works identically in a real server and in a test.
pub async fn require_auth(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let secret = req
        .app_data::<web::Data<AppConfig>>()
        .expect("AppConfig should be registered as app_data")
        .jwt
        .secret
        .clone();

    let claims = match extract_claims(&req, &secret) {
        Ok(claims) => claims,
        Err(_) => {
            let response = AppError::Unauthorized.error_response();
            return Ok(req.into_response(response).map_into_boxed_body());
        }
    };

    req.extensions_mut().insert(claims);

    next.call(req).await.map(|res| res.map_into_boxed_body())
}

fn extract_claims(
    req: &ServiceRequest,
    secret: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    crate::auth::validate_jwt(token, secret).map(|data| data.claims)
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

    use crate::config::{DatabaseConfig, JwtConfig, ServerConfig};

    fn test_app_config(secret: &str) -> AppConfig {
        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
            },
            database: DatabaseConfig {
                url: "postgres://unused".to_string(),
            },
            jwt: JwtConfig {
                secret: secret.to_string(),
            },
        }
    }

    #[actix_web::test]
    async fn rejects_a_request_with_no_authorization_header() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(test_app_config("test-secret")))
                .wrap(from_fn(require_auth))
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn rejects_a_token_signed_with_a_different_secret() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(test_app_config("test-secret")))
                .wrap(from_fn(require_auth))
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let token = crate::auth::create_jwt("42", "a-different-secret");
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn accepts_a_valid_token_and_exposes_its_claims_to_the_handler() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(test_app_config("test-secret")))
                .wrap(from_fn(require_auth))
                .route(
                    "/",
                    web::get().to(|claims: web::ReqData<Claims>| async move {
                        HttpResponse::Ok().body(claims.sub.clone())
                    }),
                ),
        )
        .await;

        let token = crate::auth::create_jwt("42", "test-secret");
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), actix_web::http::StatusCode::OK);

        let body = test::read_body(res).await;
        assert_eq!(body, "42");
    }
}
