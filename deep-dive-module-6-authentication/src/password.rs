//! Password hashing -- Module 6's applied-for-real lesson
//! (`examples::module_6::password_hashing`), used by
//! `routes::auth::authenticate_user` before a single line of JWT code runs.
//! A login endpoint is only as trustworthy as this file.

use argon2::{password_hash, Argon2, PasswordHasher, PasswordVerifier};

/// Hashes a plaintext password. The returned string embeds the algorithm,
/// its parameters, and a random salt -- everything [`verify_password`]
/// needs is in this one string; nothing extra has to be stored alongside
/// it. The salt itself is generated internally (via `argon2`'s default
/// `getrandom`-backed feature) -- there's no salt to generate or thread
/// through by hand.
pub fn hash_password(password: &str) -> Result<String, password_hash::Error> {
    let hash = Argon2::default().hash_password(password.as_bytes())?;
    Ok(hash.to_string())
}

/// Verifies a plaintext password against a previously-hashed one. Never
/// compare a raw password (or two hashes) with `==` -- always go through
/// this. A malformed `hash` (not something [`hash_password`] produced)
/// counts as "doesn't match," not a caller error -- see
/// `routes::auth::authenticate_user`, the one real caller, which treats
/// both the same way.
pub fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_correct_password_verifies() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn a_wrong_password_does_not_verify() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn hashing_the_same_password_twice_produces_different_hashes() {
        // Different random salt each time -- proves the salt isn't a fixed
        // constant, which would defeat the whole point of salting.
        let first = hash_password("same password").unwrap();
        let second = hash_password("same password").unwrap();
        assert_ne!(first, second);
        assert!(verify_password("same password", &first));
        assert!(verify_password("same password", &second));
    }

    #[test]
    fn a_malformed_hash_does_not_verify_and_does_not_panic() {
        assert!(!verify_password("anything", "not-a-real-hash"));
    }
}
