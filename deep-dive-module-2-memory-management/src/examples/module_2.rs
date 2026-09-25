//! Standalone teaching snippets for *Deep Dive: Rust* Module 2 ("Advanced
//! Memory Management"), one submodule per code sample in the lesson --
//! same structure as [`crate::examples::module_1`].
//!
//! The *applied* version of these ideas lives in the real HTTP API: see
//! [`crate::cache::RecentTasksCache`] (an `Arc<Mutex<_>>`, the thread-safe
//! answer to 2.1's `Rc<RefCell<_>>`) and [`crate::models::task::TaskSummary`]
//! (a lifetime-annotated struct, 2.2's first sample, applied for real).

/// 2.1 #1 -- `Box<T>` for Heap Allocation.
pub mod box_heap_allocation {
    /// A singly-linked list node. Without `Box`, `next: Option<ListNode>`
    /// would make `ListNode` an infinitely-sized type (a `ListNode` contains
    /// a `ListNode` contains a `ListNode`, forever) -- `Box` breaks the
    /// cycle by storing the next node on the heap, so `ListNode` itself is
    /// always just "a value plus one pointer" in size, no matter how long
    /// the chain gets.
    pub struct ListNode {
        pub value: i32,
        pub next: Box<Option<ListNode>>,
    }

    pub fn collect_values(mut current: &ListNode) -> Vec<i32> {
        let mut values = vec![current.value];
        while let Some(node) = current.next.as_ref() {
            values.push(node.value);
            current = node;
        }
        values
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn walks_the_whole_chain() {
            let node3 = ListNode {
                value: 3,
                next: Box::new(None),
            };
            let node2 = ListNode {
                value: 2,
                next: Box::new(Some(node3)),
            };
            let node1 = ListNode {
                value: 1,
                next: Box::new(Some(node2)),
            };
            assert_eq!(collect_values(&node1), vec![1, 2, 3]);
        }
    }
}

/// 2.1 #2 -- `Rc<T>` for Shared Ownership.
pub mod rc_shared_ownership {
    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::rc::Rc;

        #[test]
        fn clones_share_the_same_allocation_and_count_owners() {
            let task = Rc::new(Task {
                id: 1,
                title: "Learn Rust".into(),
            });
            let clone1 = Rc::clone(&task);
            let clone2 = Rc::clone(&task);

            assert_eq!(task.id, clone1.id);
            assert_eq!(task.id, clone2.id);
            assert_eq!(Rc::strong_count(&task), 3);

            drop(clone1);
            assert_eq!(Rc::strong_count(&task), 2);
        }
    }
}

/// 2.1 #3 -- `RefCell<T>` for Interior Mutability.
pub mod refcell_interior_mutability {
    use std::cell::RefCell;

    pub struct MockDatabase {
        queries: RefCell<Vec<String>>,
    }

    impl MockDatabase {
        pub fn new() -> Self {
            Self {
                queries: RefCell::new(Vec::new()),
            }
        }

        // `&self`, not `&mut self` -- this is the whole point of `RefCell`:
        // mutation happens through a shared reference.
        pub fn execute_query(&self, query: &str) {
            self.queries.borrow_mut().push(query.to_string());
        }

        pub fn queries(&self) -> Vec<String> {
            self.queries.borrow().clone()
        }
    }

    impl Default for MockDatabase {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn records_queries_through_a_shared_reference() {
            let db = MockDatabase::new();
            db.execute_query("SELECT * FROM tasks");
            db.execute_query("INSERT INTO tasks VALUES (1, 'Learn Rust')");
            assert_eq!(
                db.queries(),
                vec![
                    "SELECT * FROM tasks".to_string(),
                    "INSERT INTO tasks VALUES (1, 'Learn Rust')".to_string(),
                ]
            );
        }

        // `RefCell`'s borrow rules are Rust's usual "one writer XOR many
        // readers" rule, just checked while the program runs instead of
        // while it's compiling -- this is the runtime cost of interior
        // mutability. Violate it and you get a panic, not a compile error.
        #[test]
        #[should_panic]
        fn a_second_overlapping_mutable_borrow_panics() {
            let db = MockDatabase::new();
            let _first = db.queries.borrow_mut();
            let _second = db.queries.borrow_mut();
        }
    }
}

/// 2.1 #4 -- Combining `Rc` and `RefCell`.
pub mod combining_rc_refcell {
    pub struct TaskStore {
        pub tasks: Vec<String>,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::cell::RefCell;
        use std::rc::Rc;

        #[test]
        fn every_clone_can_mutate_the_same_store() {
            let store = Rc::new(RefCell::new(TaskStore { tasks: vec![] }));
            let store_clone = Rc::clone(&store);

            store.borrow_mut().tasks.push("Task 1".to_string());
            store_clone.borrow_mut().tasks.push("Task 2".to_string());

            assert_eq!(store.borrow().tasks, vec!["Task 1", "Task 2"]);
            assert_eq!(Rc::strong_count(&store), 2);
        }
    }
}

/// 2.1 #5 -- "Use in To-Do App," exactly as the lesson doc writes it, plus
/// the fix this codebase actually needed.
///
/// **Read `thread_safe` below before copying `single_threaded` anywhere.**
/// This compiles and its test passes, but `single_threaded::TaskStore` is
/// deliberately never touched by `crate::routes` or `crate::main` -- Actix
/// runs handlers across a pool of OS threads, and `Rc<RefCell<T>>` is not
/// safe to share across threads (neither type implements `Send`). The real,
/// production version of "a shared mutable list of recent tasks" is
/// [`crate::cache::RecentTasksCache`], which is this exact shape with `Rc`
/// swapped for `Arc` and `RefCell` swapped for `Mutex`.
pub mod refactor_todo_app {
    /// The lesson's version: fine for a single-threaded program (a CLI, a
    /// script, a `fn main`), unsound to reach for in a multi-threaded web
    /// server.
    pub mod single_threaded {
        use std::cell::RefCell;
        use std::rc::Rc;

        #[derive(Debug, Clone)]
        pub struct Task {
            pub id: u32,
            pub title: String,
            pub completed: bool,
        }

        pub struct TaskStore {
            pub tasks: Vec<Rc<RefCell<Task>>>,
        }

        impl TaskStore {
            pub fn new() -> Self {
                Self { tasks: Vec::new() }
            }

            pub fn add_task(&mut self, task: Task) {
                self.tasks.push(Rc::new(RefCell::new(task)));
            }

            pub fn complete_task(&self, id: u32) {
                for task in &self.tasks {
                    if task.borrow().id == id {
                        task.borrow_mut().completed = true;
                        break;
                    }
                }
            }
        }

        impl Default for TaskStore {
            fn default() -> Self {
                Self::new()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn completes_the_matching_task_only() {
                let mut store = TaskStore::new();
                store.add_task(Task {
                    id: 1,
                    title: "Learn Rust".into(),
                    completed: false,
                });
                store.add_task(Task {
                    id: 2,
                    title: "Build a project".into(),
                    completed: false,
                });

                store.complete_task(1);

                assert!(store.tasks[0].borrow().completed);
                assert!(!store.tasks[1].borrow().completed);
            }
        }
    }

    /// The fix: same shape, `Arc` instead of `Rc` (atomic, thread-safe
    /// reference counting) and `Mutex` instead of `RefCell` (a lock, so only
    /// one thread holds the borrow at a time instead of a runtime-checked
    /// single-threaded rule).
    pub mod thread_safe {
        use std::sync::{Arc, Mutex};

        #[derive(Debug, Clone)]
        pub struct Task {
            pub id: u32,
            pub title: String,
            pub completed: bool,
        }

        pub struct TaskStore {
            pub tasks: Vec<Arc<Mutex<Task>>>,
        }

        impl TaskStore {
            pub fn new() -> Self {
                Self { tasks: Vec::new() }
            }

            pub fn add_task(&mut self, task: Task) {
                self.tasks.push(Arc::new(Mutex::new(task)));
            }

            pub fn complete_task(&self, id: u32) {
                for task in &self.tasks {
                    let mut task = task.lock().unwrap();
                    if task.id == id {
                        task.completed = true;
                        break;
                    }
                }
            }
        }

        impl Default for TaskStore {
            fn default() -> Self {
                Self::new()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use std::thread;

            // Proves it's actually thread-safe, not just superficially
            // similar to the single-threaded version: the mutation happens
            // from a spawned OS thread, not the test's own thread. Swap
            // `Arc<Mutex<_>>` for `Rc<RefCell<_>>` here and this test stops
            // compiling -- `thread::spawn` requires its closure to be
            // `Send`, and `Rc`/`RefCell` opt out of that on purpose.
            #[test]
            fn completes_the_matching_task_from_another_thread() {
                let mut store = TaskStore::new();
                store.add_task(Task {
                    id: 1,
                    title: "Learn Rust".into(),
                    completed: false,
                });

                let task_handle = Arc::clone(&store.tasks[0]);
                thread::spawn(move || {
                    task_handle.lock().unwrap().completed = true;
                })
                .join()
                .unwrap();

                assert!(store.tasks[0].lock().unwrap().completed);
            }
        }
    }
}

/// 2.2 #1 -- Lifetime Annotations in Structs.
pub mod lifetime_annotations_in_structs {
    /// `'a` ties `part`'s lifetime to whatever string `Excerpt` was built
    /// from: the compiler will refuse to let an `Excerpt` outlive the
    /// `String` it borrows from.
    pub struct Excerpt<'a> {
        pub part: &'a str,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn borrows_a_slice_of_the_original_string() {
            let novel = String::from("Call me Ishmael. Some years ago...");
            let first_sentence = Excerpt { part: &novel[..16] };
            assert_eq!(first_sentence.part, "Call me Ishmael.");
        }
    }
}

/// 2.2 #2 -- Lifetime Bounds in Traits.
///
/// See `crate::models::traits::Summarize` for the idiom you'll actually
/// reach for day to day: an *elided* lifetime tied to `&self`, rather than
/// a lifetime parameter on the trait itself. This explicit form earns its
/// keep specifically when, like `NewsArticle` here, the implementor holds
/// borrowed fields directly and the returned reference should be tied to
/// *that* data's lifetime, not to how long the `&self` borrow happens to
/// last.
pub mod lifetime_bounds_in_traits {
    pub trait Summary<'a> {
        fn summarize(&self) -> &'a str;
    }

    pub struct NewsArticle<'a> {
        pub headline: &'a str,
        pub content: &'a str,
    }

    impl<'a> Summary<'a> for NewsArticle<'a> {
        fn summarize(&self) -> &'a str {
            self.headline
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn summarize_returns_the_headline() {
            let article = NewsArticle {
                headline: "Rust is awesome!",
                content: "Rust is a systems programming language...",
            };
            assert_eq!(article.summarize(), "Rust is awesome!");
        }
    }
}
