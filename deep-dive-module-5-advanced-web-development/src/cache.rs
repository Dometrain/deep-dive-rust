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

struct Inner {
    capacity: usize,
    ids: Mutex<VecDeque<TaskId>>,
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

    /// Records a newly created task id as the most recent. The lock is held
    /// only for the few instructions inside this function -- it's released
    /// as soon as `record` returns, so it never spans an `.await` point.
    pub fn record(&self, id: TaskId) {
        let mut ids = self.0.ids.lock().unwrap();
        if ids.len() == self.0.capacity {
            ids.pop_back();
        }
        ids.push_front(id);
    }

    /// Most-recently-recorded id first.
    pub fn snapshot(&self) -> Vec<TaskId> {
        self.0.ids.lock().unwrap().iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_lists_most_recent_first() {
        let cache = RecentTasksCache::new(10);
        cache.record(TaskId(1));
        cache.record(TaskId(2));
        cache.record(TaskId(3));

        assert_eq!(cache.snapshot(), vec![TaskId(3), TaskId(2), TaskId(1)]);
    }

    #[test]
    fn oldest_entry_is_evicted_once_capacity_is_reached() {
        let cache = RecentTasksCache::new(2);
        cache.record(TaskId(1));
        cache.record(TaskId(2));
        cache.record(TaskId(3));

        assert_eq!(cache.snapshot(), vec![TaskId(3), TaskId(2)]);
    }

    #[test]
    fn a_clone_shares_the_same_underlying_cache() {
        let cache = RecentTasksCache::new(10);
        let clone = cache.clone();

        clone.record(TaskId(1));

        // Recorded through the clone, visible through the original -- same
        // `Arc<Inner>` underneath, exactly like two `Rc::clone`s of the same
        // `Rc<RefCell<_>>` in the single-threaded lesson version.
        assert_eq!(cache.snapshot(), vec![TaskId(1)]);
    }

    #[test]
    fn stays_correct_when_recorded_from_another_thread() {
        let cache = RecentTasksCache::new(10);
        let cache_clone = cache.clone();

        std::thread::spawn(move || {
            cache_clone.record(TaskId(1));
        })
        .join()
        .unwrap();

        assert_eq!(cache.snapshot(), vec![TaskId(1)]);
    }
}
