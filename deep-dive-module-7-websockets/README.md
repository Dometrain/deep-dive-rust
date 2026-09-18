# Deep Dive: Rust -- Module 7: WebSockets & a Real Frontend

The same to-do API as
[`deep-dive-module-6-authentication`](../deep-dive-module-6-authentication)
-- this module **starts from that codebase and keeps building on it**, the
same way every module has since Module 2. Every type, trait, smart pointer,
FFI call, concurrency primitive, middleware, and the whole auth layer Modules
1 through 6 added is still here, unchanged. This module adds a real-time
WebSocket feed (`actix-ws` + a `tokio::sync::broadcast` channel) and then a
**browser client that consumes it** -- a Leptos single-page app written in
Rust, sharing the exact wire types the backend serializes. These are the
topics of *Deep Dive: Rust*, Module 7.

## What's in this folder

This module is three cooperating crates, not one:

| Path | Crate | What it is |
|---|---|---|
| `.` (root) | `todo-ws-app` | The backend -- Modules 1-6's app, plus `/ws` and the broadcast fan-out. Native. |
| `shared/` | `todo-shared` | The JSON wire contract shared by both ends. No `sqlx`/`actix`/FFI, so it compiles to WASM. |
| `frontend/` | `todo-ws-frontend` | The Leptos CSR browser app. Built with `trunk`, targets `wasm32`. |

They are deliberately **not** a single Cargo workspace: the native backend and
the WASM frontend target different platforms, and keeping them as independent
crates (joined only by a `path` dependency on `shared`) means a plain `cargo
build` in the root never tries to build the browser app for the server's
target. See the two `.NET`-parallel notes in `mod-7.md` if that split feels
unfamiliar.

## Prerequisites

- Rust (stable, edition 2021)
- A C compiler on `PATH` -- `cc`, `gcc` or `clang`. Module 3's FFI feature
  (the `ETag` checksum) is still part of the backend, so `build.rs` still
  links a small C library at build time.
  - macOS: `xcode-select --install`. Debian/Ubuntu: `apt install build-essential`.
- A container runtime for Postgres and the integration tests:
  [OrbStack](https://orbstack.dev) or Docker.
- **For the frontend only:** the WASM toolchain (installed once):
  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install trunk
  ```

## Running the backend

```sh
docker compose up -d db
cargo run
```

The server listens on `http://127.0.0.1:8080`. The `/tasks` CRUD endpoints and
`/login` are unchanged from Modules 1 through 6 (every `/tasks` route still
requires a bearer token). New in this module: a `GET /ws` WebSocket route, and
`POST /tasks` now broadcasts the created task to every connected client.

## Registering & logging in

A fresh database has no users. The easiest way to get one is `POST /register`
(inherited from Module 6 -- it hashes the password, inserts the row, and logs
you straight in by returning a token):

```sh
# Create an account and get a token in one call -- 201 Created:
curl -s -X POST http://127.0.0.1:8080/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}'
# {"token":"eyJ..."}

# Registering the same username again is a clean 409, not a 500:
# {"error":"Username already taken"}
```

`POST /login` takes the same `{username, password}` shape and returns the same
`{token}` for a user that already exists. Use a token on a protected route:

```sh
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}' | sed 's/.*"token":"//;s/".*//')
curl -s http://127.0.0.1:8080/tasks -H "Authorization: Bearer $TOKEN"
```

> **Tokens expire after 30 minutes** (`ACCESS_TOKEN_TTL_MINUTES` in
> `src/auth.rs`) -- if a request starts returning `401` later, just log in
> again.

### Alternative: seeding a user directly in SQL

You rarely need this now that `/register` exists, but it's handy for bulk
seeding or pinning a known hash. You can't insert a plaintext password --
`password_hash` holds an Argon2 PHC string that `password::verify_password`
reads the algorithm and parameters out of -- so insert a pre-hashed one (the
single-quoted `'SQL'` heredoc stops your shell mangling the `$`-signs):

```sh
docker compose exec -T db psql -U postgres -d todo <<'SQL'
INSERT INTO users (username, password_hash)
VALUES ('alice', '$argon2id$v=19$m=19456,t=2,p=1$bXq/o1yoQP7tHViHbQ5HXQ$2cGTNsDtiPR1402xdJj4yROmI6+Qp2IeKkZPwtVv8UI');
SQL
```

> That hash is `password123`, generated with this app's own
> `password::hash_password`. `tests/common/mod.rs`'s `insert_user` uses the
> same insert shape.

## Running the frontend

In a second terminal, with the backend already running:

```sh
cd frontend
trunk serve --port 9000 --open
```

`trunk serve` builds the WASM bundle, serves it (on a port *different* from the
backend's 8080), and live-reloads on edits. Register (or log in) with `alice` /
`password123` from [Registering & logging in](#registering--logging-in) above.
Open the page in two windows and add a task in one: it appears in both
immediately, over `/ws`. See [`frontend/README.md`](frontend/README.md) for
more.

## What's new in Module 7

| Change | Added by |
|---|---|
| A `GET /ws` WebSocket route, built on `actix-ws` (not the deprecated `actix-web-actors`), authenticated via a `?token=` query param | This module -- `src/routes/websocket.rs` |
| A per-user `Broadcaster` fan-out over `tokio::sync::broadcast`, registered as `web::Data` | This module -- `src/broadcast.rs` |
| `POST /tasks` broadcasts the created task to *that user's* connected clients only | This module -- `routes::tasks::create_task` |
| Tasks scoped to the authenticated user (every create/list/get/update/delete filters by `user_id`; another user's task id returns the same `404` as a missing one) | Applied retrospectively -- migration `20240301000000_add_task_owner.sql`, `src/db/queries.rs`, `src/routes/tasks.rs`, plus per-user scoping in `src/cache.rs`, `src/audit.rs`, and `src/broadcast.rs` |
| A `shared` wire-type crate, the single source of truth for both ends | This module -- `shared/` |
| A Leptos CSR frontend consuming `/login`, `/tasks`, and `/ws` | This module -- `frontend/` |
| A consumer-driven contract test guarding the two `Task` shapes from drifting | This module -- `tests/contract_test.rs` |
| Dev CORS now allows the browser's request headers (`.allow_any_header()`) | This module -- `src/main.rs` |

You can watch the feed without the frontend, too -- any WebSocket client
works, but the handshake now needs a token (a browser can't send an
`Authorization` header on a WebSocket upgrade, so it travels as `?token=`).
With [`websocat`](https://github.com/vi/websocat):

```sh
# Log in first to get a token:
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}' | jq -r .token)
# Then connect, passing the token in the query string:
websocat "ws://127.0.0.1:8080/ws?token=$TOKEN"
# ...then, in another terminal, create a task as the *same* user, and watch
# the JSON for it appear on the websocat connection. A task created by a
# different user never arrives here -- the feed is scoped per user.
```

## A hard-won lesson: subscribe *before* you spawn

`routes::websocket::websocket_handler` calls `broadcaster.subscribe()`
**synchronously, before** `actix_web::rt::spawn` -- not inside the spawned
task. That ordering is load-bearing: because the handler returns the handshake
response only *after* subscribing, a client's connection is already a
subscriber by the time its handshake completes. Subscribe *inside* the spawned
task instead and you open a gap -- a broadcast fired in the instant between
"handshake done" and "task scheduled, subscribed" is silently lost. It's also
exactly what lets the frontend's broadcast-delivery test be race-free: the
test can broadcast the moment `ws_at(...)` resolves and know the subscription
already exists.

## A second hard-won lesson: why the frontend can't just reuse `models::Task`

The obvious full-stack move is "share the `Task` struct." You can't share
*this* one: `models::Task` derives `sqlx::FromRow`/`sqlx::Type`, its
`TaskTitle` validation returns `AppError` (which pulls in actix), and its
`etag()` calls into C through Module 3's FFI sample. None of that compiles to
`wasm32-unknown-unknown`, and none of it means anything in a browser. So the
shared crate holds only the *flat wire view* -- exactly the JSON both ends
exchange. The cost of two definitions is that they can drift; the fix is
`tests/contract_test.rs`, which round-trips the backend's `Task` through JSON
into the shared one and fails `cargo test` if a field ever stops matching --
turning a runtime, in-browser parse error into a red build.

## A third hard-won lesson: CORS is the seam between two origins

The frontend (`trunk serve`) and the API run on different origins, so the
browser's `Authorization` and `Content-Type` request headers only survive the
CORS preflight if the backend allows them. The dev `Cors` builder in `main.rs`
gained `.allow_any_header()` for exactly this -- development only, the same
caveat as the `allow_any_origin()` that was already there. Leave it out and
every authenticated request from the browser fails at preflight, before it
ever reaches a handler, with no obvious error in the server log.

## Where each lesson sample lives

| Lesson | Submodule / file | Sample |
|---|---|---|
| 7.1 | `examples::module_7::{websocket_basics, setting_up_actix_ws}` | No code -- see `mod-7.md` and this README |
| 7.1 | `routes::websocket` | The shipped handler + its `responds_to_a_ping_with_a_pong` test |
| 7.1 | `routes::websocket::tests` | Testing against a real socket (`actix_test::start` + `awc`) |
| 7.2 | `broadcast::Broadcaster` | The `tokio::sync::broadcast` fan-out, with its own tests |
| 7.2 | `routes::websocket::websocket_handler` | The `tokio::select!` loop racing client frames vs. broadcasts |
| 7.2 | `routes::tasks::create_task` | Broadcasting the created task, applied for real |
| 7.3 | `todo-shared` (`shared/`) | The single wire-type source of truth |
| 7.3 | `todo-ws-frontend` (`frontend/`) | The Leptos CSR app + the browser end of `/ws` |
| 7.3 | `tests/contract_test.rs` | The consumer-driven contract test across the language boundary |

## Running the tests

Unit tests (WebSocket handler, `broadcast`, plus everything Modules 1-6
already tested) don't need a database, but they do need a C compiler on `PATH`
the first time. The WebSocket tests bind a real loopback socket:

```sh
cargo test --lib
```

The contract test is pure serde -- no database, no container:

```sh
cargo test --test contract_test
```

The `Send`/`Sync` compile-failure demo from Module 4 runs as a doc test:

```sh
cargo test --doc
```

Full integration tests use [Testcontainers RS](https://testcontainers.com/)
for a throwaway Postgres container per test:

```sh
cargo test
```

Type-check the frontend against the WASM target (no browser needed):

```sh
cd frontend && cargo check --target wasm32-unknown-unknown
```

> **OrbStack users:** if the tests fail with
> `SocketNotFoundError("/var/run/docker.sock")`, either enable the
> `/var/run/docker.sock` symlink in OrbStack -> Settings -> Docker, or set
> `export DOCKER_HOST="unix://$HOME/.orbstack/run/docker.sock"`.

## Docker

```sh
docker compose up --build
```

The Jaeger UI is available at <http://localhost:16686>. The builder image
(`rust:1.88-bookworm`) already ships `gcc`, so no host C compiler is needed
for the Docker build. `APP_JWT_SECRET` is a placeholder in
`docker-compose.yml`/`Dockerfile`, the same as `APP_DATABASE_URL` -- fine for
this course, never for a real deployment. (The Docker image builds and runs
the backend only; the frontend is a separate `trunk` build.)

## Exercises

Not pre-solved here -- try them against this codebase. The full set (with
hints) is in `mod-7.md`; the highlights:

1. Broadcast `update_task` and `delete_task` too -- and decide what a *delete*
   notification looks like, since there's no `Task` left to serialize.
2. Make the backend depend on `shared` and convert at the edge, so there is
   *one* `Task` definition instead of two kept in sync by a contract test.
3. Protect `/ws` with `middleware::require_auth` -- then solve the wrinkle that
   a browser `WebSocket` can't send an `Authorization` header.
4. On `RecvError::Lagged(n)`, send the client a `{"missed": n}` message and
   have the frontend re-fetch `GET /tasks` instead of trusting its local state.
5. Add optimistic UI to the frontend's "Add" button, then reconcile against
   the broadcast that follows.
