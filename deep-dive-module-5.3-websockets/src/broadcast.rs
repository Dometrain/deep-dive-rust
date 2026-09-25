//! A per-user `tokio::sync::broadcast` fan-out -- Module 7's broadcasting
//! lesson (7.2 #1), applied for real, with the per-user scoping Module 6's
//! auth now demands. Registered once as `web::Data<Broadcaster>` in
//! `main.rs`, the exact same "one shared handle, cloned cheaply per
//! worker/connection" pattern `cache::RecentTasksCache` and
//! `audit::AuditLogger` already use.
//!
//! The actor-based approach this module deliberately avoids (see the module's
//! `mod-7.md`) needed a hand-maintained `HashSet`/`Vec` of live connections,
//! added to on connect and pruned on disconnect. `broadcast` does the fan-out
//! *and* the cleanup for you: every connection calls [`Broadcaster::subscribe`]
//! for its own `Receiver` (see `routes::websocket`), and dropping that
//! `Receiver` when the connection's task ends is the entire cleanup step --
//! no registry, no `Mutex` over a connection set, no disconnect bookkeeping
//! to keep in sync.
//!
//! Per-user scoping adds exactly one wrinkle on top: instead of one channel
//! for everyone, there's one channel *per user*, kept in a small map. A
//! user with no live connections has no entry; the first connect creates it
//! and the last disconnect leaves a `Sender` behind (cheap -- a `Sender` and
//! an empty buffer, no task) until the process exits. Pruning those stale
//! senders would be real work for no real benefit in this app, so they're
//! left alone: the lesson is "drop the `Receiver` and you're done," not
//! "maintain a live-connection registry," and that stays true per channel.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

/// How many not-yet-delivered messages a slow subscriber can fall behind by
/// before it starts missing some (surfaced to the connection task as
/// `RecvError::Lagged` -- see `routes::websocket`). This is a per-subscriber
/// backlog bound, **not** a limit on how many clients can connect.
const CHANNEL_CAPACITY: usize = 16;

/// Cheap to clone: cloning copies the `Arc`, not the map of channels, so
/// every clone drives the same per-user channels -- the same shared-ownership
/// idea as `RecentTasksCache`/`AuditLogger`.
#[derive(Clone, Default)]
pub struct Broadcaster {
    channels: Arc<Mutex<HashMap<i32, broadcast::Sender<String>>>>,
}

impl Broadcaster {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Sends `message` to every currently-subscribed connection belonging to
    /// `user_id` -- and *only* those. Another user's live feed never carries
    /// this message, which is what keeps `POST /tasks`'s real-time path from
    /// undoing the per-user scoping `GET /tasks` already enforces. An `Err`
    /// (or a missing channel for `user_id`) only means there are no
    /// subscribers right now -- not a failure this app needs to handle any
    /// differently than "nobody was listening," so it's deliberately
    /// swallowed.
    pub fn broadcast(&self, user_id: i32, message: String) {
        let Ok(channels) = self.channels.lock() else {
            return;
        };
        if let Some(sender) = channels.get(&user_id) {
            let _ = sender.send(message);
        }
    }

    /// A fresh `Receiver` for `user_id` that only sees messages sent *after*
    /// this call -- a client connecting late gets live updates from that
    /// point on, never a replay of what already aired. Creates the per-user
    /// channel on first connect; later connects subscribe to the same
    /// `Sender`, so every connection for one user shares one fan-out point.
    pub fn subscribe(&self, user_id: i32) -> broadcast::Receiver<String> {
        let mut channels = self
            .channels
            .lock()
            .expect("the channels lock is never poisoned -- held only for a few instructions");
        channels
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_subscriber_for_a_user_receives_that_users_broadcast() {
        let broadcaster = Broadcaster::new();
        let mut first = broadcaster.subscribe(1);
        let mut second = broadcaster.subscribe(1);

        broadcaster.broadcast(1, "task created".to_string());

        assert_eq!(first.recv().await.unwrap(), "task created");
        assert_eq!(second.recv().await.unwrap(), "task created");
    }

    #[tokio::test]
    async fn a_broadcast_only_reaches_the_user_it_was_sent_to() {
        // The core of the per-user scoping rule over the live path: a task
        // created by one user's `POST /tasks` reaches that user's open
        // connections and no one else's.
        let broadcaster = Broadcaster::new();
        let mut alice = broadcaster.subscribe(1);
        let mut bob = broadcaster.subscribe(2);

        broadcaster.broadcast(1, "alice's task".to_string());

        assert_eq!(alice.recv().await.unwrap(), "alice's task");

        // Bob's feed stays empty -- a `recv` with a short timeout proves
        // nothing was delivered, rather than just not-yet-delivered.
        tokio::select! {
            biased;
            message = bob.recv() => panic!("bob should not receive alice's task, got {message:?}"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
        }
    }

    #[tokio::test]
    async fn a_late_subscriber_does_not_see_earlier_messages() {
        // Proves the "no replay" semantics: a `Receiver` created after a
        // message was sent never sees it, only what comes next.
        let broadcaster = Broadcaster::new();
        broadcaster.broadcast(1, "missed this one".to_string());

        let mut late = broadcaster.subscribe(1);
        broadcaster.broadcast(1, "sees this one".to_string());

        assert_eq!(late.recv().await.unwrap(), "sees this one");
    }

    #[test]
    fn broadcasting_to_a_user_with_no_subscribers_does_not_panic() {
        // The `Err(no subscribers)` / no-channel case `broadcast` deliberately
        // swallows.
        let broadcaster = Broadcaster::new();
        broadcaster.broadcast(1, "nobody is listening".to_string());
    }
}
