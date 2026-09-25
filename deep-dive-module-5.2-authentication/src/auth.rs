//! JWT generation and validation -- Module 6's applied-for-real lesson
//! (`examples::module_6::{token_generation, token_validation}`).
//!
//! [`Claims`] is the one shared type both `create_jwt` (used by
//! `routes::auth::login`) and `middleware::require_auth` decode into --
//! keeping a single definition is what stops the two from drifting apart,
//! unlike an early draft of this lesson that defined it twice.

use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};

/// How long an access token this app issues stays valid. Short on purpose
/// -- see this module's README on why a long-lived token is a real
/// liability, not just a style preference.
pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject -- who this token is for (the user id)
    pub iat: usize,  // Issued at (Unix timestamp, seconds)
    pub exp: usize,  // Expiration time (Unix timestamp, seconds)
}

/// Signs a short-lived token for `user_id`.
pub fn create_jwt(user_id: &str, secret: &str) -> String {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::minutes(ACCESS_TOKEN_TTL_MINUTES)).timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    // A plain string secret always encodes successfully -- `encode` can
    // only fail for a structurally malformed key, which can't happen
    // here. Matches this app's existing `.expect(...)`-for-unreachable
    // idiom (see `middleware::timing`), rather than threading a
    // `Result` through for a failure mode that can't occur.
    .expect("a well-formed HS256 secret always encodes successfully")
}

/// Validates `token` against `secret` and returns its claims. `HS256` is
/// pinned explicitly, not left to whatever algorithm the token's own header
/// claims -- see this module's README on why that distinction matters.
/// `decode` also checks `exp` automatically; an expired token comes back as
/// `Err` here, with no manual timestamp comparison required.
pub fn validate_jwt(
    token: &str,
    secret: &str,
) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_freshly_created_token_validates() {
        let token = create_jwt("42", "test-secret");
        let claims = validate_jwt(&token, "test-secret").unwrap().claims;
        assert_eq!(claims.sub, "42");
    }

    #[test]
    fn a_token_signed_with_a_different_secret_is_rejected() {
        let token = create_jwt("42", "test-secret");
        assert!(validate_jwt(&token, "a-different-secret").is_err());
    }

    #[test]
    fn a_token_expires_after_its_ttl() {
        let now = Utc::now();
        // Well past `jsonwebtoken`'s default 60-second leeway (for small
        // clock skew between issuer and verifier) -- a token expired by
        // only a minute would still validate under that leeway, which
        // isn't what this test is checking.
        let already_expired = Claims {
            sub: "42".to_string(),
            iat: (now - Duration::hours(2)).timestamp() as usize,
            exp: (now - Duration::hours(1)).timestamp() as usize,
        };
        let token = encode(
            &Header::default(),
            &already_expired,
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();

        assert!(validate_jwt(&token, "test-secret").is_err());
    }

    #[test]
    fn garbage_input_is_rejected_without_panicking() {
        assert!(validate_jwt("not.a.jwt", "test-secret").is_err());
    }
}
