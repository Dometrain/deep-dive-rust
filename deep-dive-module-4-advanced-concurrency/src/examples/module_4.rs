//! Standalone teaching snippets for *Deep Dive: Rust* Module 4 ("Advanced
//! Concurrency"), one submodule per code sample in the lesson -- same
//! structure as [`crate::examples::module_3`].
//!
//! Lesson 4.1 builds up native-thread concurrency in a straight line:
//! spawn, join, message-pass over a channel, then share state directly with
//! `Arc<Mutex<T>>` -- and, underneath all of it, the `Send`/`Sync` traits
//! that are the actual reason any of this is safe to write without a data
//! race. Lesson 4.2 revisits the same shapes (spawn, channel, shared state)
//! under Tokio, and adds `tokio::select!` for racing two async operations
//! against each other.
//!
//! The *applied* version of this lesson lives in the real HTTP API:
//! [`crate::audit::AuditLogger`] (background thread + channel + shared
//! state, joined on shutdown, same shape as `background_task_processor`
//! below) and `routes::tasks::get_task`'s `tokio::select!` timeout
//! (same shape as `racing_tasks_with_select` below).

/// 4.0 -- Closures: the building block every `spawn` below takes.
///
/// Before threads, a few minutes on closures themselves -- because the one
/// keyword that trips people up here (`move`) is the same keyword in front of
/// almost every `thread::spawn`/`tokio::spawn` in this module. `Fn`/`FnMut`/
/// `FnOnce` just describe *how* a closure touches what it captured: borrow,
/// borrow-mut, or own. `move` forces capture by value, which is exactly what
/// makes the borrow checker accept a closure whose spawned work may outlive
/// the stack frame that created it.
pub mod closures {
    /// A closure that only *reads* its capture implements `Fn` -- callable
    /// many times. Higher-order functions take these as `impl Fn(..) -> ..`
    /// (the same shape `actix`'s `from_fn` middleware and `tokio::select!`
    /// branches rely on).
    pub fn multiplier(factor: i32) -> impl Fn(i32) -> i32 {
        // `move` is required: `factor` must live inside the returned closure,
        // not in this stack frame.
        move |x| x * factor
    }

    /// A closure that *mutates* its capture is `FnMut`; taking it as `mut f`
    /// is what lets us call it more than once here.
    pub fn run_twice(mut f: impl FnMut()) {
        f();
        f();
    }

    /// Demonstrates `move` capturing by value -- the returned `String` was
    /// built from a value the closure took ownership of. This is the same
    /// ownership transfer that `thread::spawn(move || ...)` relies on so a
    /// thread can outlive the code that started it.
    pub fn into_greeting(name: String) -> String {
        let greet = move || format!("hello, {name}");
        greet()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn an_fn_closure_reads_its_capture_and_is_callable_repeatedly() {
            let triple = multiplier(3);
            assert_eq!(triple(2), 6);
            assert_eq!(triple(5), 15); // callable again -- it only reads `factor`
        }

        #[test]
        fn an_fnmut_closure_mutates_across_calls() {
            let mut total = 0;
            // The closure binding is `mut` because it mutates a capture.
            run_twice(|| total += 10);
            assert_eq!(total, 20);
        }

        #[test]
        fn a_move_closure_owns_its_capture() {
            let name = "alice".to_string();

            let greeting = into_greeting(name);

            assert_eq!(greeting, "hello, alice");
        }
    }
}

/// 4.1 #1 -- Spawning Threads.
pub mod spawning_threads {
    use std::thread;

    /// Spawns a thread that computes a value and returns it via `join()`.
    pub fn spawn_and_get_result() -> i32 {
        let handle = thread::spawn(|| {
            // Real work would go here; a plain expression is enough to
            // show a value coming back across the thread boundary.
            41 + 1
        });

        handle.join().unwrap()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn returns_the_spawned_closures_value() {
            assert_eq!(spawn_and_get_result(), 42);
        }
    }
}

/// 4.1 #2 -- Thread Joining.
pub mod thread_joining {
    use std::thread;

    /// Spawns five threads, each returning its own index, and collects
    /// every result via `join()` -- proving every thread actually finished
    /// before this function returns, not just that they were started.
    pub fn spawn_five_and_collect() -> Vec<i32> {
        let handles: Vec<_> = (0..5).map(|i| thread::spawn(move || i)).collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn collects_every_threads_result_in_spawn_order() {
            assert_eq!(spawn_five_and_collect(), vec![0, 1, 2, 3, 4]);
        }
    }
}

/// 4.1 #3 -- Channel Basics.
pub mod channel_basics {
    use std::sync::mpsc;
    use std::thread;

    /// Sends two messages from one spawned thread and collects both on the
    /// caller via `for received in receiver`.
    pub fn send_two_and_collect() -> Vec<String> {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            sender.send("Hello from thread!".to_string()).unwrap();
            sender.send("Another message".to_string()).unwrap();
        });

        receiver.into_iter().collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn receives_both_messages_in_send_order() {
            assert_eq!(
                send_two_and_collect(),
                vec!["Hello from thread!", "Another message"]
            );
        }
    }
}

/// 4.1 #4 -- Channel Iteration.
pub mod channel_iteration {
    use std::sync::mpsc;
    use std::thread;

    /// Spawns five threads, each sending one message through a cloned
    /// `Sender`, then drains every message on the caller.
    ///
    /// The original `Sender` is dropped once every clone has been handed to
    /// a thread -- `receiver`'s iterator only ends once *every* sender,
    /// original and clones alike, has gone out of scope. Forgetting to drop
    /// even one clone somewhere is the most common way to make a loop like
    /// this hang forever.
    pub fn fan_in_five_messages() -> Vec<String> {
        let (sender, receiver) = mpsc::channel();

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let sender = sender.clone();
                thread::spawn(move || {
                    sender.send(format!("Message {i}")).unwrap();
                })
            })
            .collect();
        drop(sender);

        for handle in handles {
            handle.join().unwrap();
        }

        // Five threads racing to send means no guaranteed arrival order;
        // sort before asserting so this test isn't flaky.
        let mut received: Vec<String> = receiver.into_iter().collect();
        received.sort();
        received
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn receives_all_five_messages_regardless_of_thread_scheduling_order() {
            assert_eq!(
                fan_in_five_messages(),
                vec![
                    "Message 0",
                    "Message 1",
                    "Message 2",
                    "Message 3",
                    "Message 4",
                ]
            );
        }
    }
}

/// 4.1 #5 -- Shared State with `Arc<Mutex<T>>`.
pub mod shared_state_with_arc_mutex {
    use std::sync::{Arc, Mutex};
    use std::thread;

    /// Ten threads each increment the same counter 100 times, guarded by
    /// one `Mutex` and shared via one `Arc`. If the lock ever let two
    /// threads in at once, the final count would come out below 1000.
    pub fn increment_from_ten_threads() -> i32 {
        let counter = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut num = counter.lock().unwrap();
                    *num += 1;
                    // The lock releases here, when `num` goes out of scope
                    // at the end of this block -- there's no `unlock()` to
                    // remember.
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total = *counter.lock().unwrap();
        total
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn every_increment_is_counted_exactly_once() {
            assert_eq!(increment_from_ten_threads(), 1000);
        }
    }
}

/// 4.1 #7 -- Borrowing Data Safely with Scoped Threads.
pub mod scoped_threads {
    use std::thread;

    /// Splits `numbers` into chunks and sums each chunk on its own scoped
    /// thread, borrowing `numbers` directly -- no `Arc`, no `clone`, no
    /// `move` of ownership away from the caller.
    ///
    /// `thread::scope` guarantees every thread spawned inside it finishes
    /// before the call returns, which is what lets the compiler accept a
    /// plain borrow here instead of demanding `numbers` be `'static`.
    pub fn sum_chunks(numbers: &[i32], chunk_size: usize) -> Vec<i32> {
        let mut sums = Vec::new();

        thread::scope(|scope| {
            let handles: Vec<_> = numbers
                .chunks(chunk_size)
                .map(|chunk| scope.spawn(move || chunk.iter().sum::<i32>()))
                .collect();

            for handle in handles {
                sums.push(handle.join().unwrap());
            }
        });

        sums
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sums_each_chunk_and_the_caller_still_owns_the_slice() {
            let numbers = vec![1, 2, 3, 4, 5, 6];
            assert_eq!(sum_chunks(&numbers, 2), vec![3, 7, 11]);

            // `numbers` is still usable here -- `thread::scope` guaranteed
            // every borrowing thread finished before it returned above.
            assert_eq!(numbers.len(), 6);
        }
    }
}

/// 4.1 #8 -- Background Task Processor.
pub mod background_task_processor {
    use std::sync::mpsc;
    use std::thread;

    pub struct Task {
        pub id: u32,
        pub name: String,
    }

    /// Feeds five tasks to a background worker thread over a channel, then
    /// joins the worker -- rather than guessing at a `thread::sleep`
    /// duration -- before handing back every task's processed label.
    ///
    /// See [`crate::audit::AuditLogger`] for this exact shape (background
    /// thread, channel, joined on shutdown) applied for real to this app's
    /// audit trail.
    pub fn process_five_tasks() -> Vec<String> {
        let (sender, receiver) = mpsc::channel::<Task>();

        let worker = thread::spawn(move || {
            let mut processed = Vec::new();
            for task in receiver {
                processed.push(format!("Processed task {}: {}", task.id, task.name));
            }
            processed
        });

        for i in 0..5 {
            sender
                .send(Task {
                    id: i,
                    name: format!("Task {i}"),
                })
                .unwrap();
        }

        // Signal "no more tasks" so the worker's `for task in receiver` loop
        // can end, then wait for it to actually finish and hand back its
        // result -- no `thread::sleep` guesswork about how long that takes.
        drop(sender);
        worker.join().unwrap()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn processes_every_task_in_order() {
            let processed = process_five_tasks();
            assert_eq!(processed.len(), 5);
            assert_eq!(processed[0], "Processed task 0: Task 0");
            assert_eq!(processed[4], "Processed task 4: Task 4");
        }
    }
}

/// 4.2 #1 -- Tokio Runtime.
pub mod tokio_runtime {
    /// There's nothing to "call" for this lesson beyond `#[tokio::test]`
    /// itself existing -- see `linking_a_c_library` in Module 3 for the
    /// same idea applied to a build step instead of a runtime. This
    /// function exists only so the fact "a runtime is running" is something
    /// a test can actually assert on.
    pub async fn runtime_is_available() -> bool {
        tokio::runtime::Handle::try_current().is_ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn a_tokio_runtime_is_running_this_test() {
            assert!(runtime_is_available().await);
        }
    }
}

/// 4.2 #2 -- Spawning Async Tasks.
pub mod spawning_async_tasks {
    use tokio::time::{sleep, Duration};

    /// Spawns two tasks with different sleep durations and awaits both.
    /// They run concurrently, so the caller waits roughly as long as the
    /// slower one, not the sum of both.
    pub async fn run_two_concurrently() -> (i32, i32) {
        let task1 = tokio::spawn(async {
            sleep(Duration::from_millis(50)).await;
            1
        });
        let task2 = tokio::spawn(async {
            sleep(Duration::from_millis(100)).await;
            2
        });

        (task1.await.unwrap(), task2.await.unwrap())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tokio::time::Instant;

        // `start_paused = true` gives this test Tokio's virtual clock:
        // `sleep` still suspends the task in scheduling order, but time
        // itself advances instantly to the next pending timer instead of
        // waiting in real wall-clock time. That makes the elapsed duration
        // below exact and deterministic -- no "generous bound" needed to
        // avoid flakiness on a loaded CI runner, the way a real-time
        // `std::time::Instant` assertion would need. This only works
        // because every delay in `run_two_concurrently` goes through
        // Tokio's clock (`tokio::time::sleep`); a real I/O wait wouldn't be
        // sped up by this.
        #[tokio::test(start_paused = true)]
        async fn both_tasks_complete_concurrently() {
            let start = Instant::now();
            assert_eq!(run_two_concurrently().await, (1, 2));
            // Concurrent: exactly 100ms (the slower task) under the paused
            // clock. Sequential execution would instead measure 150ms
            // (50ms + 100ms) -- this assertion would catch a regression to
            // that just as reliably as a fuzzy real-time bound, without any
            // chance of failing on a slow machine for an unrelated reason.
            assert_eq!(start.elapsed(), Duration::from_millis(100));
        }
    }
}

/// 4.2 #3 -- Async Channels.
pub mod async_channels {
    use tokio::sync::mpsc;

    /// Sends five messages over an async channel from a spawned task and
    /// receives every one of them on the caller.
    pub async fn send_and_receive_five() -> Vec<String> {
        let (sender, mut receiver) = mpsc::channel(10);

        tokio::spawn(async move {
            for i in 0..5 {
                sender.send(format!("Message {i}")).await.unwrap();
            }
        });

        let mut received = Vec::new();
        while let Some(message) = receiver.recv().await {
            received.push(message);
        }
        received
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn receives_every_message_in_send_order() {
            assert_eq!(
                send_and_receive_five().await,
                vec![
                    "Message 0",
                    "Message 1",
                    "Message 2",
                    "Message 3",
                    "Message 4",
                ]
            );
        }
    }
}

/// 4.2 #4 -- Task Scheduling.
pub mod task_scheduling {
    use tokio::time::{sleep, Duration};

    /// Schedules five tasks with staggered (and deliberately
    /// out-of-order-finishing) sleeps, and awaits every handle in the order
    /// the tasks were pushed.
    pub async fn schedule_five() -> Vec<u64> {
        let mut handles = vec![];

        for i in 0..5u64 {
            handles.push(tokio::spawn(async move {
                // Task 0 sleeps longest, task 4 sleeps least -- they finish
                // in the *opposite* order from how they're pushed below.
                sleep(Duration::from_millis((4 - i) * 20)).await;
                i
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn every_handle_is_awaited_in_the_order_it_was_pushed() {
            // Even though task 4 finishes first internally, `handle.await`
            // waits for *that specific task*, not "whichever task finishes
            // next" -- so results still come back in push order.
            assert_eq!(schedule_five().await, vec![0, 1, 2, 3, 4]);
        }
    }
}

/// 4.2 #5 -- Shared Async State.
pub mod shared_async_state {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Five async tasks each increment the same counter once, guarded by
    /// Tokio's own `Mutex` -- `std::sync::Mutex::lock()` would block the
    /// underlying worker thread while waiting for the lock; this one yields
    /// control back to the runtime instead. See
    /// `routes::tasks::get_task` for why that distinction matters
    /// in a real request handler, not just here.
    pub async fn increment_from_five_tasks() -> i32 {
        let counter = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                let mut num = counter.lock().await;
                *num += 1;
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let total = *counter.lock().await;
        total
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn every_increment_is_counted_exactly_once() {
            assert_eq!(increment_from_five_tasks().await, 5);
        }
    }
}

/// 4.2 #6 -- Racing Tasks with `tokio::select!`.
pub mod racing_tasks_with_select {
    use tokio::time::{sleep, Duration};

    #[derive(Debug, PartialEq, Eq)]
    pub enum Outcome {
        DataArrived(&'static str),
        TimedOut,
    }

    async fn fetch_data(delay: Duration) -> &'static str {
        sleep(delay).await;
        "data"
    }

    /// Races `fetch_data` against a fixed timeout and reports whichever
    /// finishes first, **cancelling** the other -- `tokio::select!` drops
    /// the losing branch's future outright, it doesn't let it run to
    /// completion in the background.
    ///
    /// `fetch_data` here is safe to cancel mid-`sleep`: dropping it has no
    /// side effect at all. That's *not* true of every future -- see
    /// "cancellation safety" in `tokio::select!`'s own documentation before
    /// wrapping a future that has one (a partially-sent write, a
    /// non-idempotent call, ...) in a race like this. `routes::tasks::
    /// get_task` uses this exact shape on a real database query instead of
    /// a simulated fetch; see the comment on `routes::tasks::with_timeout`
    /// for why that specific case is safe to cancel.
    pub async fn fetch_with_timeout(fetch_delay: Duration, timeout: Duration) -> Outcome {
        tokio::select! {
            data = fetch_data(fetch_delay) => Outcome::DataArrived(data),
            _ = sleep(timeout) => Outcome::TimedOut,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Paused clock (see `spawning_async_tasks::tests` for why): makes
        // both outcomes deterministic instead of depending on real
        // wall-clock scheduling, and runs instantly instead of actually
        // waiting out the delays below.
        #[tokio::test(start_paused = true)]
        async fn returns_data_when_the_fetch_wins() {
            let outcome =
                fetch_with_timeout(Duration::from_millis(10), Duration::from_millis(100)).await;
            assert_eq!(outcome, Outcome::DataArrived("data"));
        }

        #[tokio::test(start_paused = true)]
        async fn times_out_when_the_fetch_is_too_slow() {
            let outcome =
                fetch_with_timeout(Duration::from_millis(100), Duration::from_millis(10)).await;
            assert_eq!(outcome, Outcome::TimedOut);
        }
    }
}

/// 4.2 #7 -- Background Async Worker.
///
/// The async sibling of `background_task_processor` in 4.1: a worker that
/// runs for as long as the app does, consuming from a channel -- started
/// with `tokio::spawn` instead of `thread::spawn`, and ended by `.await`ing
/// its `JoinHandle` instead of calling `.join()` on it. This is the shape
/// real systems use to consume messages from something like a Kafka topic,
/// an SQS queue, or a NATS subject, off the request path, without blocking
/// whatever else the process is doing (here, serving HTTP requests). See
/// [`crate::consumer`] for this exact pattern, applied for real, in this
/// app's own startup sequence -- including what changes, and what doesn't,
/// if the channel below were a real message broker instead.
pub mod background_async_worker {
    use tokio::sync::mpsc;
    use tokio::time::Duration;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Message(pub u32);

    /// Consumes messages until the channel closes (every `Sender` clone
    /// dropped), simulating a small amount of async I/O -- an HTTP call, a
    /// database write -- per message, and returns every message it
    /// processed, in the order it processed them.
    async fn consume(mut messages: mpsc::UnboundedReceiver<Message>) -> Vec<Message> {
        let mut processed = Vec::new();
        while let Some(message) = messages.recv().await {
            tokio::time::sleep(Duration::from_millis(1)).await;
            processed.push(message);
        }
        processed
    }

    /// Spawns the background worker with `tokio::spawn` and returns
    /// immediately -- the same way `thread::spawn` does in 4.1, just
    /// handing back a `tokio::task::JoinHandle` instead of a
    /// `std::thread::JoinHandle`. The caller is free to keep doing other
    /// work while this consumes messages concurrently, on the same Tokio
    /// runtime.
    pub fn spawn_worker(
        messages: mpsc::UnboundedReceiver<Message>,
    ) -> tokio::task::JoinHandle<Vec<Message>> {
        tokio::spawn(consume(messages))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn processes_every_message_and_stops_once_the_sender_is_dropped() {
            let (sender, receiver) = mpsc::unbounded_channel();
            let worker = spawn_worker(receiver);

            for i in 0..5 {
                sender.send(Message(i)).unwrap();
            }
            // Dropping every `Sender` (the only one, here) is what lets
            // `consume`'s `while let Some(...)` loop end -- the exact async
            // counterpart of `channel_iteration`'s "drop the sender" step.
            drop(sender);

            // `.await`ing the `JoinHandle` is the async counterpart of
            // `std::thread::JoinHandle::join()` -- it returns once the
            // spawned task actually finishes, not before.
            let processed = worker.await.unwrap();
            assert_eq!(processed, (0..5).map(Message).collect::<Vec<_>>());
        }
    }
}
