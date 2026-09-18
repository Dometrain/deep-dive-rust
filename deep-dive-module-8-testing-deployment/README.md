# Deep Dive: Rust -- Module 8: Testing and Deployment

The same to-do API as
[`deep-dive-module-7-websockets`](../deep-dive-module-7-websockets) -- this
module **starts from that codebase and keeps building on it**, the same way
every module has since Module 2. Everything Modules 1 through 7 added is still
here, unchanged: the auth layer, per-user task scoping, the WebSocket broadcast
feed, and the Leptos frontend. This module adds the pieces that make the app
*shippable* -- a real readiness health check, a Dockerfile and Compose setup
that match what this app actually needs (not a generic template), and a CI
pipeline that runs the full test suite, lints, and builds the image on every
push. These are the topics of *Deep Dive: Rust*, Module 8.

Unlike a generic "testing and deployment" lesson, this module **does not
re-teach integration testing from scratch** -- this app has had a real,
`testcontainers`-backed integration suite since Module 1 (`tests/`), extended
by every module since. What was genuinely missing, and is this module's
content, is the health check and the deployment/CI story.

## What's in this folder

This module is three cooperating crates, plus a CI workflow:

| Path | Crate / file | What it is |
|---|---|---|
| `.` (root) | `todo-ws-app` | The backend -- Modules 1-7's app, plus `GET /health`. Native. |
| `shared/` | `todo-shared` | The JSON wire contract shared by both ends. No `sqlx`/`actix`/FFI, so it compiles to WASM. |
| `frontend/` | `todo-ws-frontend` | The Leptos CSR browser app. Built with `trunk`, targets `wasm32`. |
| `.github/workflows/ci.yml` | — | The CI pipeline (see [CI](#continuous-integration)). |

They are deliberately **not** a single Cargo workspace: the native backend and
the WASM frontend target different platforms, so keeping them as independent
crates (joined only by a `path` dependency on `shared`) means a plain `cargo
build` in the root never tries to build the browser app for the server's
target.

## Prerequisites

- Rust (stable, edition 2021)
- A C compiler on `PATH` -- `cc`, `gcc` or `clang` (Module 3's FFI `ETag`
  feature is still part of the backend, so `build.rs` links a small C library).
  - macOS: `xcode-select --install`. Debian/Ubuntu: `apt install build-essential`.
- A container runtime -- [OrbStack](https://orbstack.dev) or Docker -- for
  Postgres, the integration tests, and this module's Docker/CI content.
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

The server listens on `http://127.0.0.1:8080`. All prior endpoints are
unchanged; new in this module is a public `GET /health` (see
[The health check](#the-health-check-liveness-vs-readiness)).

## Registering & logging in

A fresh database has no users. The easiest way to get one is `POST /register`
(from Module 6 -- it hashes the password, inserts the row, and returns a token):

```sh
# Create an account and get a token in one call -- 201 Created:
curl -s -X POST http://127.0.0.1:8080/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}'
# {"token":"eyJ..."}

# Use a token on a protected route:
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}' | sed 's/.*"token":"//;s/".*//')
curl -s http://127.0.0.1:8080/tasks -H "Authorization: Bearer $TOKEN"
```

Tasks are scoped per user (Module 7): a token only ever sees its own tasks.
See [`../deep-dive-module-7-websockets/README.md`](../deep-dive-module-7-websockets/README.md)
for the SQL-seeding alternative and the WebSocket (`/ws?token=`) walkthrough.

## Running the frontend

In a second terminal, with the backend running:

```sh
cd frontend
trunk serve --port 9000 --open
```

Register or log in with `alice` / `password123`, then open two windows and add
a task in one -- it appears in both live, over `/ws`.

## What's new in Module 8

| Change | Added by |
|---|---|
| A `GET /health` readiness check -- `200` only if this instance can reach Postgres, `503` if not | This module -- `src/routes/health.rs` |
| The `app` service has a real `/health` healthcheck, alongside Postgres's `pg_isready` | This module -- `docker-compose.yml` |
| The runtime image installs `curl` so that healthcheck can poll `/health` from inside the container | This module -- `Dockerfile` |
| CI runs `fmt`, `clippy`, the full `testcontainers` suite, a frontend wasm check, and a gated image build on every push | This module -- `.github/workflows/ci.yml` |
| A **failure-path** test: `/health` reports `503` once the database is genuinely gone, not just `200` when it's up | This module -- `tests/health_test.rs` |

## The health check: liveness vs readiness

*Liveness* asks "should this process be restarted?" (cheap, dependency-free).
*Readiness* asks "should traffic be sent here right now?" -- and that's the one
that should check the database. A process can be alive while its database
connection is dead: alive, but useless.

`GET /health` is a **readiness** check. It runs `SELECT 1` -- the cheapest
possible real query, proving a connection can be acquired and a round trip
completes without touching any table -- and returns `200` or `503`:

```sh
curl -i http://127.0.0.1:8080/health
# HTTP/1.1 200 OK          (database reachable)

docker compose stop db
curl -i http://127.0.0.1:8080/health
# HTTP/1.1 503 Service Unavailable   (dependency down -- not a 500; the app
#                                     isn't broken, a dependency is unreachable)
```

It's registered outside the `/tasks` scope's `require_auth` (Module 6): a load
balancer polling this shouldn't need a bearer token. And it returns a plain
`HttpResponse`, not a `Result<_, AppError>` -- a readiness check that itself
needed the database up just to *report* the database is down would be a strange
failure mode.

> **Coming from .NET:** the same line `Microsoft.Extensions.Diagnostics.
> HealthChecks` draws -- a bare `AddHealthChecks()` liveness probe vs. one with
> `.AddNpgSql(...)` that pings the database -- and the same split Kubernetes
> makes between `livenessProbe` (restart) and `readinessProbe` (stop routing
> traffic).

## A hard-won lesson: a health check in a slim image needs a client

The Compose healthcheck shells out to `curl` -- but `debian:bookworm-slim`, the
runtime base, ships no HTTP client at all. A Dockerfile copied verbatim from a
tutorial that installs only `ca-certificates` would build an image whose
healthcheck can *never* pass. So the runtime stage installs `curl` **solely**
for the healthcheck; the app itself never calls it, and there's still no
`libpq` (sqlx's Postgres driver is pure Rust). One extra package, for the
container's benefit, not the app's.

> A related sharp edge worth knowing: when the database is genuinely down,
> `/health`'s `SELECT 1` waits on the pool's connect timeout before it can
> report `503`, so a real-outage response can be slow. Compose's `timeout: 3s`
> marks the container unhealthy regardless, but the endpoint itself is slower
> than it should be -- making `/health` fast-fail is one of the exercises.

## Docker

```sh
docker compose up --build
```

Builds the multi-stage image and starts `db` + `app` (+ Jaeger UI at
<http://localhost:16686>). The `app` service reports healthy only once
`/health` passes -- watch it flip to `healthy` in `docker compose ps` a few
seconds after start. The Dockerfile is built to match *this* app specifically:

- `COPY native ./native` and the builder image's bundled `gcc` -- without them
  `build.rs`'s `cc::Build` call fails and nothing else matters.
- `ca-certificates` (+ `curl`, for the healthcheck) is all the runtime needs --
  no `libpq`, despite what many Rust-on-Postgres tutorials show.
- A non-root `appuser` -- not the `root` a Dockerfile runs as by default.
- No `cargo test` in the build: the suite starts its own Postgres containers,
  so it belongs in CI, not an image build.

`APP_JWT_SECRET` is a placeholder in `docker-compose.yml`/`Dockerfile` -- fine
for this course, never for a real deployment. The image builds and runs the
backend only; the frontend is a separate `trunk` build.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and PR:

- **`backend`** -- `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings`, and `cargo test` (the *real* `testcontainers` suite -- GitHub's
  `ubuntu-latest` runners already have Docker, so a throwaway Postgres starts
  per test exactly as it does locally, no CI-specific DB setup).
- **`frontend`** -- `cargo fmt --check` and `cargo check --target
  wasm32-unknown-unknown`.
- **`docker`** -- `needs: backend`, then `docker build` -- a build whose tests
  are failing shouldn't produce an image anyone deploys.

> The workflow lives at this module's own root because each Deep Dive module is
> framed as its own standalone repository. In this combined course repo it
> won't trigger from the monorepo root (GitHub only reads `.github/workflows`
> at the repository root).

## Where each lesson sample lives

| Lesson | File | Sample |
|---|---|---|
| 8.1 | `src/routes/health.rs` | Liveness vs. readiness, and the `/health` endpoint (`SELECT 1` → `200`/`503`) |
| 8.1 | `tests/health_test.rs` | Testing the *failure* path against a real container |
| 8.2 | `Dockerfile` | A multi-stage build matching this app (C dep, non-root, no libpq, +curl for the healthcheck) |
| 8.2 | `docker-compose.yml` | `depends_on: condition: service_healthy` + the app's own `/health` healthcheck |
| 8.2 | `.github/workflows/ci.yml` | fmt, clippy, the full suite, a wasm check, and a gated image build |

## Running the tests

```sh
cargo test --lib     # fast, no database (WebSocket, broadcast, models, ...)
cargo test --doc     # Module 4's Send/Sync compile-failure demo
cargo test           # full suite incl. health_test -- needs Docker/OrbStack
cd frontend && cargo check --target wasm32-unknown-unknown   # frontend
```

`tests/health_test.rs` is the new one: it starts Postgres, asserts `/health`
returns `200`, then stops the container out from under the pool and asserts
`503` -- the failure path is the whole point, since a `/health` only ever
called while the DB is up could always return `200` regardless of what it
checked.

> **OrbStack users:** if the tests fail with
> `SocketNotFoundError("/var/run/docker.sock")`, enable the
> `/var/run/docker.sock` symlink in OrbStack -> Settings -> Docker, or set
> `export DOCKER_HOST="unix://$HOME/.orbstack/run/docker.sock"`.

## Exercises

Not pre-solved here. The full set (with hints) is in `mod-8.md`; the highlights:

1. **Split liveness from readiness:** add a dependency-free `GET /health/live`
   (just `200`) alongside the DB-checking `/health/ready`, and explain which a
   Kubernetes `livenessProbe` vs. `readinessProbe` should use, and why the
   wrong one for either is a real problem.
2. **Make `/health` fast-fail:** wrap its `SELECT 1` in a short
   `tokio::time::timeout` (reuse Module 4's lesson) so a real outage returns
   `503` in milliseconds, not after the pool's connect timeout.
3. **Run the real image:** `docker compose up --build` and confirm the `app`
   service reaches `healthy` and answers `/health`.
4. **Push the image to a registry:** extend the `docker` job to push to
   `ghcr.io` with `docker/login-action` + `docker/build-push-action`, gated to
   `main` only.
5. **Deploy it somewhere real:** deploy the image to a platform that runs
   containers from a registry (Fly.io, Render) and confirm `/health` responds
   from the public instance.
