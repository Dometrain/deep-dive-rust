//! Standalone teaching snippets for *Deep Dive: Rust* Module 6
//! ("Authentication"), one submodule per code sample in the lesson -- same
//! structure as [`crate::examples::module_5`].
//!
//! Unlike most earlier modules, almost every lesson sample here is left
//! empty on purpose, each with a comment explaining why: this module's whole
//! point is reusing what already exists (`AppConfig`, `AppError`, `from_fn`
//! middleware) for a new purpose, not inventing a parallel, easily-drifted
//! copy of security-sensitive code (password hashing, JWT signing) purely
//! to have something to run standalone. `crate::password`, `crate::auth`,
//! `crate::middleware::require_auth` and `routes::auth` already *are* this
//! lesson, applied for real, and already have their own tests. The one
//! genuinely new idea this module introduces with nothing else covering it
//! -- token storage as an HTTP-only cookie -- gets a real, tested sample
//! below.

/// 6.1 #1 -- JWT Basics.
///
/// No code to run for this one -- it's the `xxxxx.yyyyy.zzzzz` structure
/// itself, not an API. See this module's README for the full explanation
/// (and the wristband analogy).
pub mod jwt_basics {}

/// 6.1 #2 -- Password Hashing with `argon2`.
///
/// No new code here on purpose: `crate::password::{hash_password,
/// verify_password}` already is this lesson, applied for real, with its own
/// tests (`correct password verifies`, `wrong password doesn't`, `same
/// password hashes differently each time`, `malformed hash doesn't
/// verify`). A standalone copy here would just be an untested duplicate of
/// security-sensitive code that already exists.
pub mod password_hashing {}

/// 6.1 #3 -- Token Generation.
///
/// No new code here on purpose: `crate::auth::create_jwt` already is this
/// lesson, applied for real -- see its own tests.
pub mod token_generation {}

/// 6.1 #4 -- Token Validation.
///
/// No new code here on purpose: `crate::auth::validate_jwt` already is this
/// lesson, applied for real -- see its own tests, including the one proving
/// a token signed with a different secret is rejected.
pub mod token_validation {}

/// 6.2 #1 -- Extending `AppError`.
///
/// No new code here on purpose: `AppError::Unauthorized` (see `error.rs`)
/// already is this lesson, applied for real, the same way every earlier
/// module's error variant was added directly to the real type rather than
/// demonstrated on a throwaway copy.
pub mod extending_app_error {}

/// 6.2 #2 -- Config-Driven JWT Secret.
///
/// No new code here on purpose: `AppConfig`'s `jwt` field (see `config.rs`)
/// already is this lesson, applied for real, the same way Module 1's
/// `AppConfig` itself was never demonstrated on a throwaway copy either.
pub mod config_driven_secret {}

/// 6.2 #3 -- JWT Middleware (`from_fn`).
///
/// No new code here on purpose: `crate::middleware::require_auth` already
/// is this lesson, applied for real, with its own tests (rejects no
/// header, rejects a token signed with the wrong secret, accepts a valid
/// token and exposes its claims to the handler downstream).
pub mod jwt_middleware {}

/// 6.2 #4 -- User Model & Login Endpoint.
///
/// No new code here on purpose: `models::User`/`LoginRequest`/
/// `LoginResponse` and `routes::auth::login` already are this lesson,
/// applied for real -- see `tests/auth_test.rs` for end-to-end coverage
/// (correct credentials, wrong password, unknown username, and proof the
/// stored hash never appears in a response).
pub mod user_model_and_login {}

/// 6.2 #5 -- Protecting Routes.
///
/// No new code here on purpose: `routes::tasks::configure`'s
/// `.wrap(from_fn(crate::middleware::require_auth))` around the `/tasks`
/// scope (and *not* around `/login`) already is this lesson, applied for
/// real -- see `tests/tasks_test.rs`'s
/// `a_request_with_no_token_is_rejected_before_it_reaches_a_handler`.
pub mod protecting_routes {}

/// 6.2 #6 -- Token Storage & Its Tradeoffs.
///
/// This app stores tokens in the `Authorization` header, client-managed --
/// so, unlike every sample above, there's no applied cookie-based version
/// anywhere else in this codebase. This is a genuinely standalone sample.
pub mod token_storage {
    use actix_web::cookie::{Cookie, SameSite};

    /// Builds the token cookie this module's README describes -- `http_only`
    /// (blocks JavaScript from reading it, closing the XSS hole an
    /// `Authorization`-header-in-`localStorage` approach has) and
    /// `same_site(Lax)` (stops the browser from attaching it to most
    /// cross-site requests, closing most of the CSRF hole `http_only`
    /// itself doesn't close). `secure` is left to the caller: `true` in any
    /// real deployment (HTTPS-only), `false` here only so this sample's own
    /// test can build one without a TLS connection in play.
    pub fn build_token_cookie(token: String, secure: bool) -> Cookie<'static> {
        Cookie::build("token", token)
            .http_only(true)
            .secure(secure)
            .same_site(SameSite::Lax)
            .finish()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_cookie_is_http_only_and_same_site_lax() {
            let cookie = build_token_cookie("a.b.c".to_string(), true);

            assert_eq!(cookie.value(), "a.b.c");
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.same_site(), Some(SameSite::Lax));
            assert_eq!(cookie.secure(), Some(true));
        }
    }
}
