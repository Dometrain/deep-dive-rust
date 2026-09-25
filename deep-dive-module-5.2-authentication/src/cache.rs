//! A small, in-memory "recently created tasks" cache -- Module 2's `Rc` +
//! `RefCell` lesson (`examples::module_2::combining_rc_refcell`), applied
//! for real.
//!
//! Actix runs handlers across a pool of OS threads, so anything stored in
//! `web::Data` has to be `Send + Sync`. `Rc<RefCell<T>>` -- the pair the
//! lesson uses -- is neither: `Rc`'s reference count isn't updated
//! atomically, and `RefCell`'s borrow tracking isn't safe to touch from two
//! threads at once. This cache uses the thread-safe equivalents instead:
//! `Arc` (atomic reference counting -- like the `DbPool = Arc<PgPool>` this
//! codebase has used since Module 9) for shared ownership, and `Mutex` (a
//! lock: only one thread holds the data at a time) in place of `RefCell`
//! for interior mutability.
//!
//! See `examples::module_2::refactor_todo_app` for the same `Rc<RefCell<_>>`
//! -> `Arc<Mutex<_>>` swap made explicit, side by side, outside the app.

use crate::models::TaskId;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// `(user_id, TaskId)` -- the owner travels with every entry, because the
/// entries are what `GET /tasks/recent` serves, and a user's recent list
/// must not contain (or hint at) anyone else's tasks.
type OwnedTaskId = (i32, TaskId);

struct Inner {
    capacity: usize,
    ids: Mutex<VecDeque<OwnedTaskId>>,
}

/// Cheap to clone: cloning copies the `Arc`, not the queue behind it, so
/// every clone shares the same underlying cache -- the same "shared
/// ownership" idea as `Rc::clone`, just safe to hand to a different thread.
#[derive(Clone)]
pub struct RecentTasksCache(Arc<Inner>);

impl RecentTasksCache {
    pub fn new(capacity: usize) -> Self {
        Self(Arc::new(Inner {
            capacity,
            ids: Mutex::new(VecDeque::with_capacity(capacity)),
        }))
    }

    /// Records a newly created task as the most recent, with the user who
    /// created it. The lock is held only for the few instructions inside
    /// this function -- it's released as soon as `record` returns, so it
    /// never spans an `.await` point.
    pub fn record(&self, user_id: i32, id: TaskId) {
        let mut ids = self.0.ids.lock().unwrap();
        if ids.len() == self.0.capacity {
            ids.pop_back();
        }
        ids.push_front((user_id, id));
    }

    /// `user_id`'s most-recently-recorded ids, newest first. Other users'
    /// entries are invisible -- not just omitted from the response, but
    /// never observable through this API at all. One consequence of the
    /// single global window: the capacity bounds *everyone's* recent
    /// activity combined, so a user's own slice can be shorter than the
    /// full capacity when others are creating tasks too. That's fine for
    /// this cache's purpose (a quick "what did I just do" list, not a
    /// query -- the durable truth is `GET /tasks`); keeping one window for
    /// all users is also what keeps the `Arc<Mutex<VecDeque>>` lesson
    /// shape intact, rather than growing a per-user map here.
    pub fn snapshot(&self, user_id: i32) -> Vec<TaskId> {
        self.0
            .ids
            .lock()
            .unwrap()
            .iter()
            .filter(|(owner, _)| *owner == user_id)
            .map(|(_, id)| *id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_lists_most_recent_first() {
        let cache = RecentTasksCache::new(10);
        cache.record(1, TaskId(1));
        cache.record(1, TaskId(2));
        cache.record(1, TaskId(3));

        assert_eq!(cache.snapshot(1), vec![TaskId(3), TaskId(2), TaskId(1)]);
    }

    #[test]
    fn oldest_entry_is_evicted_once_capacity_is_reached() {
        let cache = RecentTasksCache::new(2);
        cache.record(1, TaskId(1));
        cache.record(1, TaskId(2));
        cache.record(1, TaskId(3));

        assert_eq!(cache.snapshot(1), vec![TaskId(3), TaskId(2)]);
    }

    #[test]
    fn a_users_snapshot_hides_other_users_entries() {
        let cache = RecentTasksCache::new(10);

        // Two users interleaving their creates: the global window sees all
        // of them, but each user's snapshot sees only their own.
        cache.record(1, TaskId(1));
        cache.record(2, TaskId(2));
        cache.record(1, TaskId(3));

        assert_eq!(cache.snapshot(1), vec![TaskId(3), TaskId(1)]);
        assert_eq!(cache.snapshot(2), vec![TaskId(2)]);
        assert_eq!(cache.snapshot(3), Vec::<TaskId>::new());
    }

    #[test]
    fn a_clone_shares_the_same_underlying_cache() {
        let cache = RecentTasksCache::new(10);
        let clone = cache.clone();

        clone.record(1, TaskId(1));

        // Recorded through the clone, visible through the original -- same
        // `Arc<Inner>` underneath, exactly like two `Rc::clone`s of the same
        // `Rc<RefCell<_>>` in the single-threaded lesson version.
        assert_eq!(cache.snapshot(1), vec![TaskId(1)]);
    }

    #[test]
    fn stays_correct_when_recorded_from_another_thread() {
        let cache = RecentTasksCache::new(10);
        let cache_clone = cache.clone();

        std::thread::spawn(move || {
            cache_clone.record(1, TaskId(1));
        })
        .join()
        .unwrap();

        assert_eq!(cache.snapshot(1), vec![TaskId(1)]);
    }
}
