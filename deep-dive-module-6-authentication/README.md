# Deep Dive: Rust -- Module 6: Authentication

The same to-do API as
[`deep-dive-module-5-advanced-web-development`](../deep-dive-module-5-advanced-web-development)
-- this module **starts from that codebase and keeps building on it**, the
same way Module 5 started from
[`deep-dive-module-4-advanced-concurrency`](../deep-dive-module-4-advanced-concurrency).
Every type, trait, smart pointer, FFI call, concurrency primitive and
middleware Modules 1 through 5 added is still here, unchanged. This module
adds password hashing (`argon2`), JWT generation and validation
(`jsonwebtoken`), a `from_fn` auth middleware protecting `/tasks`, and a
`/login` endpoint -- the topics of *Deep Dive: Rust*, Module 6.

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
are unchanged from Modules 1 through 5 -- see Module 1's README for the full
table and `curl` examples -- **except every one of them now requires a
bearer token.**

## What's new in Module 6

| Change | Added by |
|---|---|
| Passwords are hashed with `argon2`, never stored or compared as plaintext | This module -- `src/password.rs` |
| JWTs are generated and validated with `jsonwebtoken` | This module -- `src/auth.rs` |
| Every route under `/tasks` requires a valid bearer token | This module -- `middleware::require_auth`, wrapped around the `/tasks` scope in `routes::tasks::configure` |
| `POST /register` creates an account (hashing the password) and returns a token | This module -- `routes::auth::register` |
| `POST /login` authenticates a user and returns a token | This module -- `routes::auth::login` |
| Tasks are scoped to the authenticated user: every create/list/get/update/delete filters by `user_id`, and another user's task id returns the same `404` as a nonexistent one | This module -- migration `20240301000000_add_task_owner.sql`, `src/db/queries.rs`, `src/routes/tasks.rs`, plus per-user scoping in `src/cache.rs` and `src/audit.rs` |
| `AppError` gained `Unauthorized` and `Conflict` variants | This module -- `error.rs` |
| `AppConfig` gained a `jwt.secret` field, overridable via `APP_JWT_SECRET` | This module -- `config.rs` |

```sh
# Register a new account -- hashes the password, stores the user, and returns
# a token in one call (201 Created):
curl -s -X POST http://127.0.0.1:8080/register \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "correct horse battery staple"}'
# {"token":"eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...."}
#
# Registering a username that already exists is a clean 409, not a 500:
# {"error":"Username already taken"}

# Log in as an existing user (same request/response shape as /register):
curl -s -X POST http://127.0.0.1:8080/login \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "correct horse battery staple"}'
# {"token":"eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...."}

# Without a token: rejected before it reaches a handler
curl -i http://127.0.0.1:8080/tasks
# HTTP/1.1 401 Unauthorized
# {"error":"Authentication failed"}

# With a token
curl -i http://127.0.0.1:8080/tasks \
  -H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...."
# HTTP/1.1 200 OK
# [...]
#
# Each token only ever sees its own tasks. If alice creates task 1 and bob
# creates task 2, alice's GET /tasks/2 comes back 404 ("Task not found") --
# the same response as a genuinely missing id, so the endpoint can't be used
# to probe which ids belong to someone else.
```

## Where each lesson sample lives

Run every standalone sample with:

```sh
cargo test --lib
```

| Lesson | Submodule | Sample |
|---|---|---|
| 6.1 | `examples::module_6::jwt_basics` | No code -- see this README's "What's new" curl walkthrough and the module's own README-level explanation of JWT structure |
| 6.1 | `examples::module_6::password_hashing` | No new code -- see below |
| 6.1 | `examples::module_6::token_generation` | No new code -- see below |
| 6.1 | `examples::module_6::token_validation` | No new code -- see below |
| 6.2 | `examples::module_6::extending_app_error` | No new code -- see below |
| 6.2 | `examples::module_6::config_driven_secret` | No new code -- see below |
| 6.2 | `examples::module_6::jwt_middleware` | No new code -- see below |
| 6.2 | `examples::module_6::user_model_and_login` | No new code -- see below |
| 6.2 | `examples::module_6::protecting_routes` | No new code -- see below |
| 6.2 | `examples::module_6::token_storage` | Builds the `http_only` + `same_site(Lax)` token cookie this README's "Token storage" section describes, and asserts its attributes |

## A note on this module: almost everything is "applied for real," on purpose

Unlike most earlier modules, `examples::module_6` is almost entirely empty
submodules with a one-line comment each. That's deliberate, not an
oversight: password hashing and JWT signing are security-sensitive code, and
writing a second, standalone "teaching" copy alongside the real
implementation is a good way to let the two quietly drift apart --
`crate::password`, `crate::auth`, `crate::middleware::require_auth` and
`routes::auth` already **are** this lesson, applied for real, and every one
of them has its own tests:

- `src/password.rs` -- `hash_password`/`verify_password`, tested against a
  correct password, a wrong password, two hashes of the same password
  (proving the salt differs each time), and a malformed hash.
- `src/auth.rs` -- `create_jwt`/`validate_jwt`, tested for a round trip, a
  token signed with the wrong secret, an expired token, and garbage input.
- `src/middleware.rs` -- `require_auth`, tested with no header, a token
  signed with the wrong secret, and a valid token whose claims actually
  reach the handler.
- `tests/auth_test.rs` -- `POST /login` end to end: correct credentials
  (and the token it returns is accepted by a real protected route), a wrong
  password, an unknown username (with an assertion that the response is
  byte-for-byte identical to the wrong-password case), and proof the stored
  hash never appears in the response body.
- `tests/tasks_test.rs` -- every existing test now carries a bearer token,
  plus two new ones: no token is rejected, and a token signed with the
  wrong secret is rejected.

The one genuinely new idea nothing else in this app covers --
cookie-based token storage -- gets a real, standalone sample:
`examples::module_6::token_storage`.

## A hard-won lesson: `from_fn` middleware and `?` don't mix the way you'd expect

Every other error in this app flows the same way: a handler returns
`Result<HttpResponse, AppError>`, and `?` propagates a failure up through it,
converted once via `AppError`'s `ResponseError` impl (see Module 5's
`error_propagation_across_layers`). The natural instinct for
`middleware::require_auth` is to write the same thing:

```rust
let claims = extract_claims(&req, &secret).map_err(|_| AppError::Unauthorized)?;
```

**This compiles, and it is wrong** -- not incorrect Rust, but a request that
never gets the `401` a client should see. A `from_fn` middleware that
short-circuits by returning `Err` from `Service::call` (rather than calling
`next` and letting a *handler's* `Result` get resolved by Actix's `Responder`
machinery) only becomes a real HTTP response via the full `HttpServer`
dispatcher -- confirmed against `actix-http`'s own H1 dispatcher source,
which converts a top-level `Err` through `ResponseError` before writing a
response. `actix_web::test::call_service`, used throughout this app's own
tests (including `require_auth`'s), skips that dispatcher entirely and calls
the built service directly -- its own doc comment says exactly this: *"Panics
if service call returns error. To handle errors use `app.call(req)`."*

In other words: the `?`-based version would have worked correctly against a
real client, and panicked in every test written the way this app's tests are
normally written. `middleware::require_auth` instead builds its `401`
response by hand:

```rust
Err(_) => {
    let response = AppError::Unauthorized.error_response();
    return Ok(req.into_response(response).map_into_boxed_body());
}
```

...and unifies that with the success path's `ServiceResponse<impl MessageBody>`
via `.map_into_boxed_body()` on both branches (Rust's `impl Trait` return
type has to resolve to one concrete type either way). This works identically
whether the request comes from a real client or `test::call_service` --
which is exactly why `middleware::tests::rejects_a_request_with_no_authorization_header`
could be written the same way every other test in this app is.

## A second hard-won lesson: a green `cargo test` isn't proof the real app runs

`cargo test` passed, completely, before this module's `main.rs` wiring was
actually correct. Running `cargo run` and hitting it with real `curl`
requests turned up a second bug `cargo test` had no way to catch: `main.rs`
registered the JWT secret as `web::Data::new(config.clone())`, where
`config: Arc<AppConfig>` (see `config::load_config`) -- and `web::Data<T>`
already Arc-wraps whatever it's given internally, so that line actually
registered `web::Data<Arc<AppConfig>>`. Every handler and
`middleware::require_auth` asks for `web::Data<AppConfig>` instead -- a type
that was simply never registered. The result in a real running server: every
request to a protected route panicked a worker thread (`AppConfig should be
registered as app_data`) and the connection dropped with no response at all;
`POST /login` failed a different way, with Actix's own `web::Data`
extractor returning a `500` before the handler even ran.

Every integration test in `tests/` missed this completely, for a subtle
reason: `tests/common::test_app_config()` builds and registers a plain
`AppConfig` directly, sidestepping the exact mismatch that existed in
`main.rs`'s own wiring. The fix is one line --
`web::Data::new(config.as_ref().clone())`, unwrapping the `Arc` once at
startup so the registered type actually matches what every consumer asks
for -- but no amount of `cargo test` would have found it, because nothing in
`tests/` exercises `main.rs` itself. That's not a gap specific to this
module; it's true of this app's `main.rs` in every module before this one
too. The practical takeaway: for anything `main.rs` wires up that isn't
independently covered elsewhere, actually run the server before calling a
feature done.

## Running the tests

Unit tests (everything in the table above, plus `middleware`, `audit`,
`routes::tasks::with_timeout`, `cache`, `ffi`, `models::task`,
`models::traits`, `auth` and `password`) don't need a database, but they do
need a C compiler on `PATH` the first time (see
[Prerequisites](#prerequisites)):

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

`APP_JWT_SECRET` is set to a placeholder value in `docker-compose.yml` and
`Dockerfile`, the same way `APP_DATABASE_URL` already was -- fine for this
course, never for a real deployment.

## Exercises

Not pre-solved here -- try them against this codebase:

1. `POST /register` (see `routes::auth::register`) creates an account and logs
   the new user straight in by returning a token. Two natural extensions: (a)
   reject a too-short password *before* hashing, with a `400` distinct from the
   `409` a duplicate username gives; and (b) make registration *not*
   auto-login -- return `201` with the created user (never the
   `password_hash`) instead of a token, so a client must `/login` separately.
   Decide which behaviour you want and adjust the tests in `tests/auth_test.rs`
   to match.
2. `auth::ACCESS_TOKEN_TTL_MINUTES` is `30`. Change it to something very
   short (like `1`), confirm (with `curl`, waiting past the expiry) that an
   old token is rejected, then move it into `AppConfig` instead of leaving
   it as a hardcoded constant -- the same way `jwt.secret` already is.
3. `middleware::require_auth`'s rejection response builds an
   `AppError::Unauthorized` regardless of *why* validation failed (missing
   header, bad signature, expired token, ...). Add a
   `tracing::debug!`-level log line inside `extract_claims`'s error path
   naming which case it was -- useful for your own debugging, still without
   changing what the client sees.
4. Add a `role: String` claim to `auth::Claims`, thread it through
   `create_jwt` and `routes::auth::login`, and write a test proving a
   decoded token's `role` matches what was encoded.
5. Design (in a comment, not necessarily working code) what server-side
   state a `/refresh` endpoint would need to be able to revoke a stolen
   refresh token before it expires. Then implement a minimal version: a
   `refresh_tokens` table with a `revoked` column, checked on every
   `/refresh` call. See `examples::module_6`'s module-level doc comment for
   why this is a deliberate, worthwhile break from "JWTs are stateless."
6. `examples::module_6::token_storage::build_token_cookie` exists but is
   never wired into a real handler -- this app stores tokens in the
   `Authorization` header instead. Add a `/login-cookie` route that sets the
   cookie instead of returning a JSON token, and explain in a comment why a
   client using it would no longer need to manage the `Authorization` header
   itself, but would now need CSRF protection it didn't need before.
