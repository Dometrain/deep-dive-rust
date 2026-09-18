# Deep Dive: Rust -- Module 4: Advanced Concurrency

The same to-do API as
[`deep-dive-module-3-unsafe-rust`](../deep-dive-module-3-unsafe-rust) --
this module **starts from that codebase and keeps building on it**, the
same way Module 3 started from
[`deep-dive-module-2-memory-management`](../deep-dive-module-2-memory-management).
Every type, trait, smart pointer and FFI call Modules 1 through 3 added
(`TaskId`, `TaskTitle`, `Displayable`, `Storable`, `RecentTasksCache`,
`TaskSummary<'a>`, `ffi::checksum_hex`, ...) is still here, unchanged. This
module adds native threads, channels, `Arc<Mutex<T>>` shared state, the
`Send`/`Sync` traits, and Tokio's async task/channel/timeout primitives --
the topics of *Deep Dive: Rust*, Module 4. That includes background
services: a `thread::spawn`ed one (`audit.rs`) and, its async sibling, a
`tokio::spawn`ed one (`consumer.rs`) modeling a Kafka-style event consumer.

## Prerequisites

- Rust (stable, edition 2021)
- A C compiler on `PATH` -- `cc`, `gcc` or `clang`. Module 3's FFI feature
  (the `ETag` checksum) is still part of this app, so `build.rs` still links
  a small C library at build time.
  - macOS: `xcode-select --install` if you've never installed the Xcode
    Command Line Tools.
  - Debian/Ubuntu: `apt install build-essential`.
  - Nothing to install if you only ever build inside Docker -- see
    [Docker](#docker).
- A container runtime for Postgres and the integration tests:
  [OrbStack](https://orbstack.dev) or Docker

## Running locally

```sh
docker compose up -d db
cargo run
```

The server listens on `http://127.0.0.1:8080`. Configuration and the
`/tasks` CRUD endpoints are unchanged from Modules 1 through 3 -- see
Module 1's README for the full table and `curl` examples.

## What's new in Module 4

| Change | Added by |
|---|---|
| `GET /tasks/audit` returns every task-creation event recorded so far | This module -- `POST /tasks` sends an event over a channel; a background thread appends it to the log |
| `GET /tasks/{id}` returns `504 Gateway Timeout` instead of hanging if the database query takes longer than 3 seconds | This module -- the query is raced against a timeout with `tokio::select!` |
| Every created task is also published to a background **async** consumer, running the whole time the app does | This module -- `POST /tasks` sends an event over a `tokio::sync::mpsc` channel; a `tokio::spawn`ed task (not a thread) consumes it -- see [`src/consumer.rs`](src/consumer.rs) |

```sh
curl -s -X POST http://127.0.0.1:8080/tasks \
  -H 'content-type: application/json' \
  -d '{"title": "Learn concurrent Rust"}'

curl -s http://127.0.0.1:8080/tasks/audit
# [{"task_id":1,"message":"created \"Learn concurrent Rust\""}]
```

Unlike the audit trail, the background async consumer has no HTTP endpoint of its own -- it's meant to be a fire-and-forget publish, the way a real message-broker producer call would be. Watch it work in the server's own logs instead:

```text
{"level":"INFO","fields":{"message":"processed task event","task_id":"1","kind":"task.created"}, ...}
```

## Where each lesson sample lives

Run every standalone sample with:

```sh
cargo test --lib
```

The one exception is the `Send`/`Sync` compile-failure demo, which is a
doc test rather than a unit test (see 4.1's last row below) -- run it with:

```sh
cargo test --doc
```

| Lesson | Submodule | Sample |
|---|---|---|
| 4.0 | `examples::module_4::closures` | `Fn`/`FnMut`/`FnOnce` and a `move` closure that owns its capture -- the building block every `spawn` below takes |
| 4.1 | `examples::module_4::spawning_threads` | `thread::spawn` running a closure, its return value collected via `join()` |
| 4.1 | `examples::module_4::thread_joining` | Five threads spawned and joined, results collected in spawn order |
| 4.1 | `examples::module_4::channel_basics` | `std::sync::mpsc`, one sender, two messages, received via `for received in receiver` |
| 4.1 | `examples::module_4::channel_iteration` | Five threads fan in through cloned senders; the receiver's loop ends once every clone is dropped |
| 4.1 | `examples::module_4::shared_state_with_arc_mutex` | Ten threads incrementing one counter through `Arc<Mutex<i32>>` -- the foundational shared-state pattern |
| 4.1 | `examples::module_4::send_and_sync` | `Arc<i32>` sent across a thread (compiles); a `compile_fail` doc test proving `Rc<i32>` can't be |
| 4.1 | `examples::module_4::scoped_threads` | `thread::scope` borrowing a local `Vec` across threads with no `Arc` |
| 4.1 | `examples::module_4::background_task_processor` | Threads + a channel processing five tasks, joined (not slept) before returning |
| 4.2 | `examples::module_4::tokio_runtime` | Confirms a Tokio runtime is actually driving an `async fn` |
| 4.2 | `examples::module_4::spawning_async_tasks` | Two `tokio::spawn`ed tasks with different sleep durations, running concurrently |
| 4.2 | `examples::module_4::async_channels` | `tokio::sync::mpsc`, one producer task, messages drained on the caller |
| 4.2 | `examples::module_4::task_scheduling` | Five staggered tasks awaited in push order regardless of internal finish order |
| 4.2 | `examples::module_4::shared_async_state` | `Arc<tokio::sync::Mutex<i32>>` -- the async-aware sibling of 4.1's `std::sync::Mutex` sample |
| 4.2 | `examples::module_4::racing_tasks_with_select` | `tokio::select!` racing a simulated fetch against a timeout, and what "cancellation safety" means for the loser |
| 4.2 | `examples::module_4::background_async_worker` | The async sibling of 4.1's `background_task_processor` -- `tokio::spawn` + a channel + `.await` on the `JoinHandle`, instead of `thread::spawn` + a channel + `.join()` |

## What's different from Module 3: why concurrency primitives stay out of most of `src/routes`

Every request this API handles before this module was: deserialize JSON,
validate a few fields, run one query, serialize JSON back -- a single
`.await` on a single database call, with no reason to spawn a thread, spawn
a task, or share state across a request boundary. That's still true for
`list_tasks`, `create_task` (mostly), `update_task` and `delete_task`. Adding
a hand-rolled `thread::spawn` or `tokio::spawn` to one of those handlers
wouldn't be "using Module 4's lesson" -- it would be manufacturing
concurrency a single sequential `.await` chain already handles correctly,
for no benefit.

So instead of forcing threads and channels into every handler, this module
adds three small, real, narrowly-scoped features -- the same shape Module
3's `ffi.rs` took with the `ETag` checksum:

- [`src/audit.rs`](src/audit.rs) is Module 4's threads + channels + shared
  state lesson, applied for real. `create_task` sends one event over an
  `std::sync::mpsc` channel and returns immediately; a single background
  thread, spawned once in [`src/main.rs`](src/main.rs), owns the receiving
  end, appends each event to an `Arc<Mutex<Vec<AuditEvent>>>`, and is
  **joined**, not abandoned, during shutdown -- see
  [`AuditWorker::shutdown`](src/audit.rs) for exactly why that join can't
  hang. `GET /tasks/audit` (in
  [`src/routes/tasks.rs`](src/routes/tasks.rs)) reads the same `Mutex`.
- [`routes::tasks::get_task`](src/routes/tasks.rs)'s `tokio::select!` timeout
  is Module 4's async lesson, applied for real: the database call is raced
  against a fixed `GET_TASK_TIMEOUT` via `with_timeout`, the same
  `tokio::select!` shape as the lesson sample
  (`examples::module_4::racing_tasks_with_select`), so one slow query
  returns a `504` instead of hanging the request indefinitely.

  In real production code you'd almost always reach for
  [`tokio::time::timeout`](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html)
  instead of hand-rolling this with `select!` -- `with_timeout`'s doc
  comment says so explicitly. It's written out by hand here specifically to
  put the lesson's own primitive to work at a real call site, not because
  that's the better choice in general.

  Racing a future against a timeout means **cancelling** whichever one
  loses -- `tokio::select!` drops the losing branch outright rather than
  letting it run to completion unseen. That's only safe to do if dropping
  the future mid-flight can't corrupt state, which isn't true of every
  future (see "cancellation safety" in `tokio::select!`'s own docs, and
  `examples::module_4::racing_tasks_with_select`'s doc comment). `with_timeout`'s
  doc comment lays out, in detail, why cancelling `db::get_task`
  specifically is safe: it's a read, and `sqlx`'s connection pool discards
  -- rather than reuses -- a connection that was dropped mid-query, so a
  cancelled query here can't leave a later request reading a corrupted
  response off the same connection. That reasoning was checked by hand, not
  just asserted: firing repeated rapid requests against a 1-nanosecond
  timeout -- guaranteeing every single query gets cancelled mid-flight --
  never produced a wrong body or a `500`, only clean `504`s.

- [`src/consumer.rs`](src/consumer.rs) is Module 4's *async* threads +
  channels lesson, applied for real -- the `tokio::spawn` sibling of
  `audit.rs`'s `thread::spawn`. `create_task` also publishes an event
  through [`EventPublisher::publish`](src/consumer.rs), a plain,
  non-`async`, non-blocking channel send -- same shape as
  `AuditLogger::record`. [`consumer::run`](src/consumer.rs) is spawned once
  in [`src/main.rs`](src/main.rs), with `tokio::spawn` instead of
  `thread::spawn`, and consumes events for as long as the app runs; at
  shutdown, `main.rs` `.await`s its `JoinHandle` -- the async counterpart of
  `AuditWorker::shutdown`'s `.join()`. See `consumer.rs`'s own doc comment
  for exactly what would change (and what wouldn't) if the channel it
  simulates were a real Kafka topic instead.

Grep this codebase's non-test, non-`examples` source for `thread::spawn`,
`tokio::spawn` or `tokio::select!` and `audit.rs`, `consumer.rs` and
`routes/tasks.rs`'s `with_timeout` are the only matches -- everything else
in the request path is still the same straight-line `.await` chain it was
in Module 3.

## Running the tests

Unit tests (everything in the table above, plus `audit`, `consumer`,
`routes::tasks::with_timeout`, `cache`, `ffi`, `models::task` and
`models::traits`) don't need a database, but they do need a C compiler on
`PATH` the first time (see [Prerequisites](#prerequisites)):

```sh
cargo test --lib
```

The `Send`/`Sync` compile-failure demo runs separately, as a doc test:

```sh
cargo test --doc
```

Full integration tests, same as every module since 9, use
[Testcontainers RS](https://testcontainers.com/) for a throwaway Postgres
container per test -- including `audit_log_records_every_created_task` in
[`tests/tasks_test.rs`](tests/tasks_test.rs), which polls `GET /tasks/audit`
a few times with a short retry rather than asserting on the very first read,
since the audit worker processes events off the request path:

```sh
cargo test
```

> **OrbStack users:** if the tests fail with
> `SocketNotFoundError("/var/run/docker.sock")`, either enable the
> `/var/run/docker.sock` symlink in OrbStack → Settings → Docker, or set:
>
> ```sh
> export DOCKER_HOST="unix://$HOME/.orbstack/run/docker.sock"
> ```

## Docker

```sh
docker compose up --build
```

The Jaeger UI is available at <http://localhost:16686>. No extra setup
needed here even if your host has no C compiler: the builder stage's base
image, `rust:1.88-bookworm`, ships `gcc` already, and `build.rs` runs inside
that container, not on your host.

## Exercises

Not pre-solved here -- try them against this codebase:

1. `examples::module_4::send_and_sync::rc_across_a_thread_does_not_compile`
   only shows the failing case in a doc comment. Run `cargo test --doc` and
   read the actual `E0277` error rustdoc captures, then write one sentence
   in a comment naming which trait is missing and why `Arc` has it but `Rc`
   doesn't.
2. `examples::module_4::shared_state_with_arc_mutex::increment_from_ten_threads`
   uses `Mutex<i32>`. Add a second function using `Arc<AtomicI32>` and
   `fetch_add(1, Ordering::SeqCst)` instead, with a test proving both reach
   1000, and explain in a comment which one you'd reach for by default and
   why.
3. `audit::AuditLogger::record` silently drops an event if the worker thread
   has already exited. Change it to return a `Result`, and add a test that
   shuts the worker down and then asserts a subsequent `record` call reports
   the failure instead of silently succeeding.
4. `routes::tasks::GET_TASK_TIMEOUT` is a hardcoded constant. Move it into
   `AppConfig` (see `config.rs`) so it's configurable via
   `config/default.toml` and an `APP_*` environment variable override, the
   same way `server.port` already is.
5. Add a `tokio::select!`-based timeout to `routes::tasks::list_tasks` too,
   reusing `with_timeout`, and a test proving a normal request still
   succeeds comfortably inside the window.
6. `examples::module_4::scoped_threads::sum_chunks` never needs `Arc`
   because `thread::scope` guarantees every thread finishes before it
   returns. Write a version that uses `thread::spawn` and `Arc<Vec<i32>>`
   instead (no `scope`), and explain in a comment exactly which guarantee
   `thread::scope` was providing that you had to replace by hand.
7. `consumer::EventPublisher::publish` silently drops an event if the
   consumer task has already exited -- the same trade-off as exercise 3
   above, for `audit::AuditLogger::record`. Apply the same fix here: change
   it to report the failure instead of swallowing it, and add a test
   proving it.
8. `consumer.rs`'s doc comment shows what a real `rdkafka` consumer loop
   would look like, but it's never compiled (the `ignore`d code block in
   `cargo doc`'s output). Pick a pure-Rust, no-native-dependencies
   alternative crate (for example `rskafka`) and sketch what `run`'s
   `receiver.recv().await` loop would become if it consumed from a real
   topic instead of the simulated channel -- you don't need a running
   Kafka broker to do this, just get it to compile against the crate's
   types.
9. `main.rs` spawns two background workers now -- `audit_worker` (a
   thread) and `event_consumer` (a task) -- and shuts each down
   differently: `audit_worker.shutdown()` (a method on a custom
   `AuditWorker` type) vs. `event_consumer.await` (directly on the
   `tokio::task::JoinHandle`, no wrapper type at all). Nothing about
   `tokio::task::JoinHandle` actually forced that difference -- `main`
   could just as well have called `.join()` directly on a bare
   `std::thread::JoinHandle` for the audit worker, the same way it awaits
   `event_consumer` directly. Read `audit::AuditWorker`'s doc comment for
   the *real* reason it exists (hint: it's about how many `AuditLogger`
   clones vs. how many workers there are, not about what `JoinHandle`
   can or can't do), then write a one-sentence comment above
   `event_consumer` explaining why it didn't need the same wrapper.
