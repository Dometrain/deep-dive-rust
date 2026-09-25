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
