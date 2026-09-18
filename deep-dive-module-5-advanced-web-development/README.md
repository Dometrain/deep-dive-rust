# Deep Dive: Rust -- Module 5: Advanced Web Development

The same to-do API as
[`deep-dive-module-4-advanced-concurrency`](../deep-dive-module-4-advanced-concurrency)
-- this module **starts from that codebase and keeps building on it**, the
same way Module 4 started from
[`deep-dive-module-3-unsafe-rust`](../deep-dive-module-3-unsafe-rust).
Every type, trait, smart pointer, FFI call and concurrency primitive
Modules 1 through 4 added (`TaskId`, `TaskTitle`, `RecentTasksCache`,
`ffi::checksum_hex`, `audit::AuditLogger`, the `tokio::select!` query
timeout, ...) is still here, unchanged. This module adds Actix-web
middleware (CORS, a custom `from_fn` middleware, and the actual order
`.wrap()` calls execute in) and `anyhow::Context`-based startup error
messages -- the topics of *Deep Dive: Rust*, Module 5.

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

The server listens on `http://127.0.0.1:8080`. The `/tasks` CRUD endpoints
are unchanged from Modules 1 through 4 -- see Module 1's README for the
full table and `curl` examples.

## What's new in Module 5

| Change | Added by |
|---|---|
| Every response carries an `X-Response-Time-Ms` header | This module -- a custom `from_fn` middleware, `middleware::timing` |
| CORS is enabled (any origin, `GET`/`POST`/`PUT`/`DELETE`, dev-only config) | This module -- `actix-cors`, wired up in `main.rs` |
| A bad config, unreachable database, or failed migration now says *which* startup step failed, instead of a generic error | This module -- `anyhow::Context` in `main.rs` |

```sh
curl -i http://127.0.0.1:8080/tasks
# HTTP/1.1 200 OK
# x-response-time-ms: 1
# ...

curl -i -X OPTIONS http://127.0.0.1:8080/tasks \
  -H "Origin: https://example.com" \
  -H "Access-Control-Request-Method: GET"
# HTTP/1.1 200 OK
# access-control-allow-origin: https://example.com
# x-response-time-ms: 0
# ...
```

That second example is worth pausing on: the `OPTIONS` preflight is
intercepted and answered entirely by `actix-cors`, before the request ever
reaches a route handler -- and it *still* comes back with an
`X-Response-Time-Ms` header. That's not an accident; see
["What's different from Module 4"](#whats-different-from-module-4-why-middleware-order-is-the-real-lesson-here)
below for why.

## Where each lesson sample lives

Run every standalone sample with:

```sh
cargo test --lib
```

| Lesson | Submodule | Sample |
|---|---|---|
| 5.1 | `examples::module_5::cors_middleware` | A real CORS preflight request, tested end-to-end, asserting the response actually carries `Access-Control-Allow-Origin` |
| 5.1 | `examples::module_5::custom_middleware` | The generic shape of an `actix_web::middleware::from_fn` middleware -- add a fixed response header |
| 5.1 | `examples::module_5::middleware_order` | Two `from_fn` middlewares recording their own execution order into a shared log -- proof, not narration, that the *last*-registered `.wrap()` runs *first* for a request |
| 5.2 | `examples::module_5::postgresql_setup` | `PgPoolOptions` + `connect_lazy` -- builds a real, working pool without a live database, and rejects a malformed URL immediately |
| 5.2 | `examples::module_5::sqlx_with_postgres` | No new code -- see below |
| 5.2 | `examples::module_5::migrations` | Reads this app's actual migration file off disk and checks its contents |
| 5.2 | `examples::module_5::connection_pooling` | `Arc`-wrapping a pool and proving cloned handles point at the same one, via `connect_lazy` again |
| 5.2 | `examples::module_5::error_context_with_anyhow` | `anyhow::Context` wrapping a deliberately simple failure (parsing a port), asserting the original error is still in the chain |
| 5.2 | `examples::module_5::error_propagation_across_layers` | `?` propagating an `AppError` from a data-access-shaped function, through a handler-shaped function, into the right HTTP status |

## A note on Lesson 5.2: this app was never on SQLite

Most *Deep Dive: Rust* cohorts using this material will be migrating a real
app from SQLite to PostgreSQL in this lesson. This codebase can't do that
demonstration, because it's been PostgreSQL-based from early in the course
-- `db::connection`, `db::queries` and `migrations/` already are lesson
5.2's PostgreSQL/SQLx/migrations/pooling content, applied for real, tested
end-to-end in [`tests/tasks_test.rs`](tests/tasks_test.rs) since long before
this module existed. There's no live migration to perform here, so
`examples::module_5::sqlx_with_postgres` is deliberately empty of new code
(see its doc comment) rather than a shorter, untested duplicate of code
that already exists and is already covered.

What genuinely *was* still missing from this app -- and is this module's
real contribution to 5.2 -- is the other half of the lesson:
`anyhow::Context` on the startup sequence. Before this module, every
startup failure (a bad config file, an unreachable database, a broken
migration) collapsed into the same generic message via
`.map_err(io::Error::other)`. See `main.rs` and the next section.

## What's different from Module 4: why middleware order is the real lesson here

This module adds three `.wrap()` calls to `main.rs`, and their order is
deliberate, not incidental:

```rust
.wrap(Cors::default()...)          // innermost -- closest to the routes
.wrap(from_fn(middleware::timing)) // middle
.wrap(TracingLogger::default())    // outermost -- registered last
```

Actix-web nests `.wrap()` layers like an onion: the **last**-registered
layer is the **outermost** one, so it sees a request first and the response
last. Registering `TracingLogger` last means it wraps everything else, so
it logs *every* request -- including one `Cors` answers itself without ever
reaching a route handler. Registering `middleware::timing` around `Cors`
(rather than inside it) is what makes the `curl` example above work: even a
CORS-short-circuited preflight response passes back up through
`middleware::timing` on its way out, so it still gets timed. Get this order
backwards -- CORS outermost, say -- and preflight requests would never
reach the logger at all, silently disappearing from your access logs.
[`examples::module_5::middleware_order`](src/examples/module_5.rs) proves
the underlying rule (last-registered-runs-first) in isolation; this section
is that rule applied to a real, three-layer stack with real consequences
for getting it wrong.

One more small, deliberate deviation from this module's own lesson sample:
the `anyhow::Context` message on the database-connection step in `main.rs`
does **not** include `config.database.url`, unlike the lesson's own
example. A Postgres connection string embeds credentials (see the
`skip_all` comment on `db::connection::create_pool`, a rule this codebase
has followed since Module 1) -- interpolating it into an error message
would be a straightforward way to leak a password into logs. "failed to
connect to the database" is enough detail to act on without that risk.

## Running the tests

Unit tests (everything in the table above, plus `middleware`, `audit`,
`routes::tasks::with_timeout`, `cache`, `ffi`, `models::task` and
`models::traits`) don't need a database, but they do need a C compiler on
`PATH` the first time (see [Prerequisites](#prerequisites)):

```sh
cargo test --lib
```

The `Send`/`Sync` compile-failure demo from Module 4 runs separately, as a
doc test:

```sh
cargo test --doc
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

The Jaeger UI is available at <http://localhost:16686>. No extra setup
needed here even if your host has no C compiler: the builder stage's base
image, `rust:1.88-bookworm`, ships `gcc` already, and `build.rs` runs inside
that container, not on your host.

## Exercises

Not pre-solved here -- try them against this codebase:

1. `main.rs` allows any origin (`allow_any_origin()`) -- fine for a course,
   not for production. Change it to `allowed_origin("http://localhost:5173")`
   (or whatever your frontend's real origin would be) and confirm, with
   `curl`, that a preflight from a *different* origin no longer gets an
   `Access-Control-Allow-Origin` header back.
2. Add a fourth `.wrap()` -- pick anything simple, like a middleware that
   rejects requests missing a `User-Agent` header -- and predict, in a
   comment, exactly where in the request/response cycle it'll run relative
   to the existing three, *before* running it. Then verify with a test
   modeled on `examples::module_5::middleware_order`.
3. `middleware::timing`'s header reports whole milliseconds
   (`.as_millis()`). For a very fast handler this rounds down to `0` a lot,
   which the CORS preflight example in this README shows happening for
   real. Switch it to microseconds and explain in a comment why that's
   more or less useful depending on what the header is actually for
   (human-readable debugging vs. feeding a metrics pipeline).
4. Move `main.rs`'s hardcoded CORS config (allowed origins, methods, max
   age) into `AppConfig` (see `config.rs`), the same way exercise 4 in
   Module 4's README moved `GET_TASK_TIMEOUT` there.
5. `examples::module_5::error_context_with_anyhow` wraps a single,
   deliberately simple failure. Extend it (or write a new sample) that
   chains **two** `.context()` calls across two function layers, and assert
   that `err.chain()` contains messages from *both* layers, not just the
   innermost one.
6. This app's own `AppError` (see `error.rs`) doesn't implement
   `std::error::Error` by hand -- `thiserror`'s `#[derive(Error)]` does it
   for you. Confirm you can `.context("...")` a `Result<T, AppError>`
   exactly the way `main.rs` does for `sqlx::Error` and `ConfigError`, with
   a small standalone test, and explain in a comment what property of
   `AppError` makes that possible.
