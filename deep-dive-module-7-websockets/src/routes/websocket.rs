//! `GET /ws` -- Module 7's applied-for-real WebSocket route, built on
//! `actix-ws` (not the deprecated `actix-web-actors`). One long-lived,
//! full-duplex connection per client: the handler completes the HTTP upgrade,
//! then hands the rest of the connection's life to a spawned task that races
//! two sources of work against each other with `tokio::select!` -- Module 4's
//! racing-tasks lesson (`examples::module_4::racing_tasks_with_select`),
//! applied again rather than reintroduced.
//!
//! Registered at the whole-`App` level in `main.rs`. Unlike `/tasks`, it is
//! *not* behind `middleware::require_auth` -- a browser's WebSocket handshake
//! can't carry an `Authorization` header, so the standard `require_auth`
//! extractor pattern doesn't apply. Instead the token travels as a query
//! parameter (`?token=...`), validated right here before the upgrade. That
//! keeps the live feed per-user: a connection only ever subscribes to its
//! owner's `Broadcaster` channel, so one user's task updates can't reach
//! another user's open tab.

use crate::{auth::validate_jwt, broadcast::Broadcaster, config::AppConfig, error::AppError};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::Message;
use futures_util::StreamExt;
use tokio::sync::broadcast;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/ws", web::get().to(websocket_handler));
}

/// Pulls the bearer token out of the query string as `?token=...`. A JWT's
/// characters are all URL-safe (base64url plus `.`), so no percent-decoding
/// is needed for the tokens this app actually mints -- which is the only
/// kind `validate_jwt` below will accept anyway.
fn token_from_query(req: &HttpRequest) -> Option<&str> {
    req.query_string()
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

async fn websocket_handler(
    req: HttpRequest,
    body: web::Payload,
    broadcaster: web::Data<Broadcaster>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, Error> {
    // Authenticate *before* the upgrade: an unauthenticated client gets a
    // plain `401` and never becomes a WebSocket at all, rather than an open
    // socket that then has to be torn down. `AppError::Unauthorized` flows
    // out as the same `{"error":"Authentication failed"}` body every other
    // auth failure in this app produces.
    let token = token_from_query(&req).ok_or(AppError::Unauthorized)?;
    let claims = validate_jwt(token, &config.jwt.secret)
        .map(|data| data.claims)
        .map_err(|_| AppError::Unauthorized)?;
    let owner: i32 = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    // `handle()` returns three things: the `HttpResponse` that completes the
    // upgrade handshake (returned below, immediately), a `Session` (the handle
    // for sending *to* this client), and a `MessageStream` (messages *from*
    // this client).
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    // Subscribe *before* spawning: this runs synchronously, so by the time the
    // handshake `response` reaches the client, this connection is already a
    // subscriber -- a broadcast fired the instant after a client connects
    // can't slip through a gap between "handshake done" and "subscribed."
    // Scoped to `owner`: this connection only ever hears about *its* user's
    // tasks (see `Broadcaster::subscribe`).
    let mut updates = broadcaster.subscribe(owner);

    // Everything below runs for the rest of the connection's life on its own
    // spawned task -- the same "hand off and return right away" shape
    // `audit::AuditLogger` uses for its background thread. Returning
    // `response` now is what actually completes the upgrade.
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // One of two things this connection might have to react to at
                // any moment: a frame *from* this client...
                client_msg = msg_stream.next() => {
                    let result = match client_msg {
                        Some(Ok(Message::Ping(bytes))) => session.pong(&bytes).await,
                        Some(Ok(Message::Close(reason))) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        // This app's clients only ever *receive* task updates;
                        // there's nothing to do with a text/binary frame they
                        // send, so it's acknowledged by doing nothing.
                        Some(Ok(_)) => Ok(()),
                        // Stream ended, or a protocol error -- either way this
                        // connection is done.
                        Some(Err(_)) | None => break,
                    };
                    // A disconnected client makes every further send return
                    // `Err(Closed)` -- breaking here, rather than `.unwrap()`ing,
                    // is what stops this task from panicking the moment a client
                    // goes away.
                    if result.is_err() {
                        break;
                    }
                }
                // ...or an update broadcast *to* this user (see
                // `routes::tasks::create_task`).
                update = updates.recv() => {
                    match update {
                        Ok(text) => {
                            if session.text(text).await.is_err() {
                                break;
                            }
                        }
                        // This connection fell more than the channel's capacity
                        // behind -- some updates were dropped *for this
                        // connection specifically*. Other subscribers that kept
                        // up are unaffected. `continue` keeps this connection
                        // alive; it just missed some updates, which is a
                        // deliberately gentler outcome than disconnecting a
                        // merely-slow client.
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
        // When the loop breaks, this task ends and `updates` (a `Receiver`) is
        // dropped -- and that drop *is* the entire cleanup step. No registry to
        // prune, no disconnect handler to keep in sync.
    });

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::create_jwt;
    use crate::config::{DatabaseConfig, JwtConfig, ServerConfig};
    use actix_test::start;
    use actix_web::App;
    use awc::ws::{Frame, Message as WsMessage};
    use futures_util::SinkExt;

    /// The signing secret both the test `AppConfig` and the test tokens share
    /// -- has to match for `validate_jwt` in the handler to accept them.
    const JWT_SECRET: &str = "ws-test-secret";

    fn test_app_config() -> AppConfig {
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
                secret: JWT_SECRET.to_string(),
            },
        }
    }

    /// A `/ws?token=...` URL for `user_id`, signed with [`JWT_SECRET`] -- the
    /// shape a real browser client uses once it has logged in.
    fn ws_url(user_id: i32) -> String {
        format!("/ws?token={}", create_jwt(&user_id.to_string(), JWT_SECRET))
    }

    #[actix_web::test]
    async fn responds_to_a_ping_with_a_pong() {
        let broadcaster = Broadcaster::new();
        let factory_broadcaster = broadcaster.clone();
        let mut server = start(move || {
            App::new()
                .app_data(web::Data::new(factory_broadcaster.clone()))
                .app_data(web::Data::new(test_app_config()))
                .configure(configure)
        });

        let mut connection = server
            .ws_at(&ws_url(1))
            .await
            .expect("handshake should succeed");
        connection
            .send(WsMessage::Ping(Vec::new().into()))
            .await
            .expect("send should succeed");

        match connection
            .next()
            .await
            .expect("a frame")
            .expect("no protocol error")
        {
            Frame::Pong(_) => {}
            other => panic!("expected a pong frame, got {other:?}"),
        }
    }

    #[actix_web::test]
    async fn a_connected_client_receives_its_own_users_broadcast() {
        // The payoff: a message pushed to the `Broadcaster` for a user (as
        // `create_task` does for real) reaches that user's client that's just
        // sitting on an open connection. The `broadcaster` handle kept out
        // here is the *same* map the app factory's clone drives, so
        // broadcasting on it fans out to every connection for that user the
        // server accepted.
        let broadcaster = Broadcaster::new();
        let factory_broadcaster = broadcaster.clone();
        let mut server = start(move || {
            App::new()
                .app_data(web::Data::new(factory_broadcaster.clone()))
                .app_data(web::Data::new(test_app_config()))
                .configure(configure)
        });

        let mut connection = server
            .ws_at(&ws_url(1))
            .await
            .expect("handshake should succeed");

        // Safe from a race: `websocket_handler` subscribes before returning the
        // handshake response, so by the time `ws_at` resolved above, this
        // connection is already subscribed and this broadcast can't be missed.
        broadcaster.broadcast(1, "a new task".to_string());

        match connection
            .next()
            .await
            .expect("a frame")
            .expect("no protocol error")
        {
            Frame::Text(bytes) => assert_eq!(bytes.as_ref(), b"a new task"),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    #[actix_web::test]
    async fn a_connected_client_does_not_receive_another_users_broadcast() {
        // The per-user scoping rule over the live path: a task created by
        // user 2 never reaches user 1's open connection.
        let broadcaster = Broadcaster::new();
        let factory_broadcaster = broadcaster.clone();
        let mut server = start(move || {
            App::new()
                .app_data(web::Data::new(factory_broadcaster.clone()))
                .app_data(web::Data::new(test_app_config()))
                .configure(configure)
        });

        let mut connection = server
            .ws_at(&ws_url(1))
            .await
            .expect("handshake should succeed");

        broadcaster.broadcast(2, "someone else's task".to_string());

        // A short timeout proves nothing was delivered, rather than just
        // not-yet-delivered.
        tokio::select! {
            biased;
            frame = connection.next() => panic!("user 1 should not receive user 2's task, got {frame:?}"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
        }
    }

    #[actix_web::test]
    async fn a_handshake_without_a_token_is_rejected() {
        let broadcaster = Broadcaster::new();
        let factory_broadcaster = broadcaster.clone();
        let mut server = start(move || {
            App::new()
                .app_data(web::Data::new(factory_broadcaster.clone()))
                .app_data(web::Data::new(test_app_config()))
                .configure(configure)
        });

        // No `?token=` -- the upgrade is refused before it happens, so `ws_at`
        // returns an error rather than an open connection.
        let result = server.ws_at("/ws").await;
        assert!(
            result.is_err(),
            "an unauthenticated handshake should not upgrade"
        );
    }

    #[actix_web::test]
    async fn a_handshake_with_a_token_signed_by_the_wrong_secret_is_rejected() {
        let broadcaster = Broadcaster::new();
        let factory_broadcaster = broadcaster.clone();
        let mut server = start(move || {
            App::new()
                .app_data(web::Data::new(factory_broadcaster.clone()))
                .app_data(web::Data::new(test_app_config()))
                .configure(configure)
        });

        let forged = create_jwt("1", "not-the-real-secret");
        let result = server.ws_at(&format!("/ws?token={forged}")).await;
        assert!(result.is_err(), "a forged token should not upgrade");
    }
}
