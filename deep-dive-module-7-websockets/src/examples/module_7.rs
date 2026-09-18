//! Standalone teaching snippets for *Deep Dive: Rust* Module 7
//! ("WebSockets"), one submodule per code sample in the lesson -- same
//! structure as [`crate::examples::module_6`].
//!
//! Like Module 6, this module is almost entirely "applied for real" rather
//! than demonstrated on throwaway copies: WebSocket session handling and the
//! broadcast fan-out are wired straight into the running app, and each real
//! implementation carries its own tests. Keeping one copy (not a parallel
//! teaching duplicate) is what stops the two from drifting apart.
//!
//! - `crate::broadcast::Broadcaster` -- the `tokio::sync::broadcast` fan-out
//!   (7.2 #1), tested for multi-subscriber delivery, the "no replay for a
//!   late subscriber" semantics, and the no-subscribers no-op.
//! - `crate::routes::websocket` -- `actix_ws::handle`, the per-connection
//!   `tokio::select!` loop (7.1 #3 + 7.2 #2), and the `create_task` wiring
//!   (7.2 #3). Tested against a *real* bound socket with `actix-test` + `awc`:
//!   a ping is answered with a pong, and a broadcast reaches a connected
//!   client.

/// 7.1 #1 -- WebSocket Basics.
///
/// No code to run for this one -- it's the protocol shape (an HTTP `Upgrade`
/// handshake, then a persistent full-duplex connection), not an API. See
/// `mod-7.md`'s "postcard vs. phone call" explanation.
pub mod websocket_basics {}

/// 7.1 #2 -- Setting Up `actix-ws`.
///
/// No new code here on purpose: the dependency choice (`actix-ws`, not the
/// deprecated `actix-web-actors`) lives in `Cargo.toml`, applied for real.
pub mod setting_up_actix_ws {}

/// 7.1 #3 -- Handling a Single Connection.
///
/// No new code here on purpose: `crate::routes::websocket::websocket_handler`
/// already is this lesson, applied for real (answering pings, never
/// `.unwrap()`ing a send in the loop) -- see its own `responds_to_a_ping_
/// with_a_pong` test.
pub mod handling_a_single_connection {}

/// 7.1 #4 -- Testing a WebSocket Route.
///
/// No new code here on purpose: `crate::routes::websocket`'s own tests
/// already are this lesson, applied for real -- the one place in this whole
/// app that reaches for `actix_test::start` + `awc` and a real bound socket,
/// because a WebSocket handshake genuinely needs one.
pub mod testing_a_websocket_route {}

/// 7.2 #1 -- A Broadcast Channel, Not a Client Registry.
///
/// No new code here on purpose: `crate::broadcast::Broadcaster` already is
/// this lesson, applied for real, with its own tests -- no `Arc<Mutex<Vec<
/// Session>>>`, no manual disconnect bookkeeping.
pub mod broadcast_channel_not_a_registry {}

/// 7.2 #2 -- Wiring the Broadcast Channel into a Connection.
///
/// No new code here on purpose: the `tokio::select!` loop in
/// `crate::routes::websocket::websocket_handler` already is this lesson,
/// applied for real -- the same macro `examples::module_4::
/// racing_tasks_with_select` taught, racing "a frame from this client"
/// against "an update for everyone," and treating `RecvError::Lagged`
/// (skip) differently from `Closed` (disconnect).
pub mod wiring_broadcast_into_a_connection {}

/// 7.2 #3 -- Applied for Real: Broadcasting Task Events.
///
/// No new code here on purpose: `routes::tasks::create_task`'s call to
/// `Broadcaster::broadcast` already is this lesson, applied for real --
/// serializing the same `Task` the REST API returns, and logging (not
/// failing the request on) a serialization error.
pub mod broadcasting_task_events {}
