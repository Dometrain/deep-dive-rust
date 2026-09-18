//! A background audit trail for task creation -- Module 4's threads,
//! channels and shared-state lesson, applied for real.
//!
//! `routes::tasks::create_task` calls [`AuditLogger::record`] and returns
//! immediately: the call is just a channel send, never a lock held across
//! an `.await`, never a database write. One background thread, spawned once
//! in `main` by [`AuditLogger::spawn`], owns the receiving end and is the
//! only thing that ever appends to the in-memory log. See `main.rs` for
//! where that thread is joined -- not abandoned -- during shutdown, and
//! `examples::module_4::background_task_processor` for the same
//! spawn-channel-join shape as a standalone lesson sample.

use crate::models::TaskId;
use serde::Serialize;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tracing::info;

/// One recorded event: which user, which task, and what happened to it.
/// The user rides along because the events are what `GET /tasks/audit`
/// serves -- without it, one user's audit trail (whose messages contain
/// task *titles*) would be readable by every other user.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub user_id: i32,
    pub task_id: TaskId,
    pub message: String,
}

/// Cheap to clone -- cloning copies the `Sender` and the `Arc`, not the log
/// itself, so every Actix worker thread gets a clone that still funnels
/// into the one background thread `AuditLogger::spawn` started.
#[derive(Clone)]
pub struct AuditLogger {
    sender: Sender<AuditEvent>,
    log: Arc<Mutex<Vec<AuditEvent>>>,
}

/// The other half of [`AuditLogger::spawn`]'s return value. Kept exactly
/// once (in `main`), never cloned, and consumed by [`AuditWorker::shutdown`]
/// -- that asymmetry (many `AuditLogger` handles, one `AuditWorker`) is what
/// makes "join the background thread on shutdown" a single, unambiguous
/// call site instead of something every clone would need to coordinate.
pub struct AuditWorker {
    handle: Option<JoinHandle<()>>,
}

impl AuditLogger {
    /// Spawns the background thread that owns the audit log and returns a
    /// `(logger, worker)` pair.
    pub fn spawn() -> (Self, AuditWorker) {
        let (sender, receiver) = mpsc::channel::<AuditEvent>();
        let log = Arc::new(Mutex::new(Vec::new()));
        let log_for_worker = Arc::clone(&log);

        // The one background thread in this app that isn't a lesson sample:
        // it drains `receiver` until every `Sender` clone (one per Actix
        // worker's `AuditLogger`, plus the one `main` holds until it's
        // moved into the `HttpServer` factory closure) has been dropped,
        // which is what lets `for event in receiver` end on its own.
        let handle = thread::spawn(move || {
            for event in receiver {
                info!(user_id = event.user_id, task_id = %event.task_id, "{}", event.message);
                log_for_worker.lock().unwrap().push(event);
            }
        });

        (
            Self { sender, log },
            AuditWorker {
                handle: Some(handle),
            },
        )
    }

    /// Records an event without blocking the caller on anything but an
    /// (unbounded) channel send -- the actual work of storing and logging
    /// the event happens on the background thread, off the request path.
    ///
    /// A failed send only means the worker thread has already exited (e.g.
    /// mid-shutdown); silently dropping the event at that point is the
    /// right call for a best-effort audit trail, not a must-not-lose-data
    /// queue. See this module's `README.md` exercise for the alternative.
    pub fn record(&self, user_id: i32, task_id: TaskId, message: String) {
        let _ = self.sender.send(AuditEvent {
            user_id,
            task_id,
            message,
        });
    }

    /// Every recorded event for `user_id`, oldest first. Other users'
    /// events are invisible to this caller -- the log itself stays shared
    /// and complete (all owners, all events) so the background thread and
    /// the tracing output still see the whole trail; scoping happens only
    /// at the read. Reads from the same `Mutex` the background thread
    /// writes through -- there's no other way to reach the `Vec` inside.
    pub fn snapshot(&self, user_id: i32) -> Vec<AuditEvent> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.user_id == user_id)
            .cloned()
            .collect()
    }
}

impl AuditWorker {
    /// Waits for the background thread to drain the channel and exit.
    ///
    /// This only returns promptly because, by the time it's called (see
    /// `main.rs`), every `Sender` clone has already been dropped -- calling
    /// this while an `AuditLogger` clone is still alive somewhere would
    /// block forever, the same "forgot to drop a sender" pitfall
    /// `examples::module_4::channel_iteration` calls out.
    pub fn shutdown(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().expect("audit worker thread panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_visible_after_the_worker_drains_them() {
        let (logger, worker) = AuditLogger::spawn();
        // Private field, visible here because `tests` is a child module of
        // `audit` -- grabbed up front so the log can still be read after
        // every `Sender`-holding `AuditLogger` clone below is dropped.
        let log = Arc::clone(&logger.log);

        logger.record(1, TaskId(1), "task 1 created".to_string());
        logger.record(1, TaskId(2), "task 2 created".to_string());

        // Dropping the only `AuditLogger` (and, with it, its `Sender`) lets
        // the worker's `for event in receiver` loop end once both events
        // above are drained.
        drop(logger);

        // Only returns once the worker thread has exited, which only
        // happens after every queued event has been processed -- so
        // there's no race to wait out here, unlike a real HTTP round trip.
        worker.shutdown();

        let recorded = log.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].task_id, TaskId(1));
        assert_eq!(recorded[1].task_id, TaskId(2));
    }

    #[test]
    fn a_users_snapshot_hides_other_users_events() {
        let (logger, worker) = AuditLogger::spawn();

        // Two users' events, interleaved into the one shared log...
        logger.record(1, TaskId(1), "user 1's task".to_string());
        logger.record(2, TaskId(2), "user 2's task".to_string());
        logger.record(1, TaskId(3), "user 1's other task".to_string());

        // ...each snapshot only ever serving its owner's slice of it.
        // Unlike the tests above, the logger is kept alive to call
        // `snapshot` on it, so the worker hasn't necessarily drained the
        // channel yet -- hence this bounded retry loop rather than a
        // single read (the same tolerance for background-thread timing
        // `tests/tasks_test.rs`'s audit test needs over real HTTP).
        let mut user_1 = logger.snapshot(1);
        for _ in 0..100 {
            if user_1.len() == 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            user_1 = logger.snapshot(1);
        }
        assert_eq!(user_1.len(), 2);
        assert!(user_1
            .iter()
            .all(|event| event.message.contains("user 1's")));

        // By the same drain, user 2's slice is exactly their one event.
        let mut user_2 = logger.snapshot(2);
        for _ in 0..100 {
            if user_2.len() == 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            user_2 = logger.snapshot(2);
        }
        assert_eq!(user_2.len(), 1);
        assert_eq!(user_2[0].task_id, TaskId(2));

        drop(logger);
        worker.shutdown();
    }

    #[test]
    fn a_clone_shares_the_same_underlying_log() {
        let (logger, worker) = AuditLogger::spawn();
        let clone = logger.clone();
        let log = Arc::clone(&logger.log);

        clone.record(1, TaskId(7), "recorded via a clone".to_string());

        // Both handles hold a `Sender` clone -- both have to go before the
        // worker's loop can end.
        drop(logger);
        drop(clone);
        worker.shutdown();

        assert_eq!(log.lock().unwrap()[0].task_id, TaskId(7));
    }
}
