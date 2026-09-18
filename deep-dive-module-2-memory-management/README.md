# Deep Dive: Rust -- Module 2: Advanced Memory Management

The same to-do API as [`deep-dive-module-1-advanced-traits`](../deep-dive-module-1-advanced-traits)
-- this module **starts from that codebase and keeps building on it**, the
same way Module 1 started from
[`module-9-final-project`](../module-9-final-project). Every type and trait
Module 1 added (`TaskId`, `TaskTitle`, `TaskDescription`, `Displayable`,
`Storable`, `Validatable`, `TaskEntity`) is still here, unchanged. This
module adds smart pointers, interior mutability and lifetimes on top --
the topics of *Deep Dive: Rust*, Module 2.

## Prerequisites

- Rust (stable, edition 2021)
- A container runtime for Postgres and the integration tests:
  [OrbStack](https://orbstack.dev) or Docker

## Running locally

```sh
docker compose up -d db
cargo run
```

The server listens on `http://127.0.0.1:8080`. Configuration and the
`/tasks` CRUD endpoints are unchanged from Module 1 -- see that module's
README for the full table and `curl` examples.

## What's new in Module 2

| Endpoint | Added by |
|---|---|
| `GET /tasks/recent` | This module -- the ids of the 10 most recently created tasks, most recent first |

```sh
curl http://127.0.0.1:8080/tasks/recent
```

## Where each lesson sample lives

Module 1's `src/examples.rs` is now `src/examples/`, one file per module
(`module_1.rs`, `module_2.rs`) -- this is meant to keep growing that way as
the course adds more modules. Run every standalone sample with:

```sh
cargo test --lib
```

| Lesson | Submodule | Sample |
|---|---|---|
| 2.1 | `examples::module_2::box_heap_allocation` | `Box<T>` for a recursive linked list |
| 2.1 | `examples::module_2::rc_shared_ownership` | `Rc<T>` for shared ownership |
| 2.1 | `examples::module_2::refcell_interior_mutability` | `RefCell<T>` for interior mutability (including the runtime-panic case) |
| 2.1 | `examples::module_2::combining_rc_refcell` | `Rc<RefCell<T>>` for shared mutable state |
| 2.1 | `examples::module_2::refactor_todo_app` | The lesson's `Rc<RefCell<Task>>` `TaskStore`, **and** the `Arc<Mutex<Task>>` fix needed to actually use it from more than one thread |
| 2.2 | `examples::module_2::lifetime_annotations_in_structs` | `Excerpt<'a>` |
| 2.2 | `examples::module_2::lifetime_bounds_in_traits` | `Summary<'a>` (a trait generic over an explicit lifetime) |
| 2.2 | `examples::module_2::lifetime_elision` | `first_word` |
| 2.2 | `examples::module_2::variance` | Covariance (tested), contravariance and invariance (explained in comments -- see why below) |

## What's different from Module 1: why `Rc<RefCell<_>>` never appears in `src/routes` or `src/main.rs`

The lesson's own "Use in To-Do App" sample wraps every task in
`Rc<RefCell<Task>>`. Wiring that into the real app the way it's written
would be a mistake, not a shortcut -- and the lesson's own "Common
Pitfalls" table agrees (*"Overusing `Rc`: using `Rc` in multi-threaded
contexts... Fix: use `Arc`"*). Actix Web runs handlers across a pool of OS
threads, so anything shared across requests (`web::Data<T>`) has to be
`Send + Sync`. Neither `Rc` nor `RefCell` is: `Rc`'s reference count isn't
updated atomically, and `RefCell`'s borrow tracking isn't safe to touch
from two threads without a data race.

Instead, [`src/cache.rs`](src/cache.rs) adds `RecentTasksCache` -- the
*real* answer to "shared mutable state across the app" -- built from `Arc`
(atomic reference counting) and `Mutex` (a lock, so only one thread holds
the data at a time). It's the exact same shape as the lesson's
`Rc<RefCell<Task>>`, just with the thread-safe equivalents, and it's the
same pattern this codebase has used since Module 9 without a name attached:
`db::DbPool` has been `Arc<PgPool>` the whole time.
[`examples::module_2::refactor_todo_app`](src/examples/module_2.rs) shows
both versions side by side, including a test that proves the `Arc<Mutex<_>>`
version really is thread-safe by mutating it from a spawned OS thread.

## What's different from Module 1: a lifetime-annotated struct, applied for real

[`Task::summary()`](src/models/task.rs) returns a `TaskSummary<'a>` --
a struct holding `&'a str`, tied to the `Task` it borrowed from, instead of
an owned `String`. It's used in `routes::tasks::create_task` when logging a
task into the recent-tasks cache, so that log line doesn't allocate.
[`Summarize`](src/models/traits.rs) is the same idea via a trait method
instead of a struct field -- and, because `Task` owns its data instead of
borrowing it, its lifetime can be *elided* (`fn summarize(&self) -> &str`)
rather than written out explicitly the way the lesson's own
`Summary<'a>` sample has to.

## Running the tests

Unit tests (everything in the tables above, plus `cache`, `models::task`
and `models::traits`) don't need a database:

```sh
cargo test --lib
```

Full integration tests, same as every module since 9, use
[Testcontainers RS](https://testcontainers.com/) for a throwaway Postgres
container per test:

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

The Jaeger UI is available at <http://localhost:16686>.

## Exercises

Not pre-solved here -- try them against this codebase:

1. Add a `Weak<Inner>` "observer" to `RecentTasksCache` that can check
   whether the cache is still alive without keeping it alive itself. What
   would need `Weak` in a real app, and why doesn't this cache?
2. `examples::module_2::box_heap_allocation::ListNode` only ever has zero
   or one child. Extend it (or write a new type) into a binary tree node
   with `Box<Option<TreeNode>>` on both sides.
3. Add a `struct TaskExcerpt<'a> { title: &'a str, description: &'a str }`
   (two borrowed fields instead of `TaskSummary`'s one) and a method on
   `Task` that returns one.
4. Rewrite `examples::module_2::lifetime_bounds_in_traits::Summary<'a>`'s
   `NewsArticle<'a>` to instead hold owned `String` fields, then change
   `summarize` to an elided lifetime like `models::traits::Summarize`. What
   has to change, and what stays the same?
5. `examples::module_2::variance`'s contravariance and invariance sections
   are comments, not tests, because proving them needs code that correctly
   *fails* to compile. Write the failing version of the `Cell<T>` example
   in a scratch file and confirm the compiler rejects it the way the
   comment says it would.
