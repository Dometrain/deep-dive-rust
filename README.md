# Deep Dive: Rust

Companion code for the *Deep Dive: Rust* course. Every module is the same
to-do API (Actix Web + SQLx + Postgres). Each module starts from the one
before it and adds that module's topic, so the app grows as you work
through the course.

Each module folder is its own standalone Cargo project with its own
`README.md` and `RECORDING_GUIDE.md`. There is no top-level Cargo workspace:
`cd` into a module folder to build, run or test it.

## Modules

| Module | Folder | Topics | What it adds to the app |
|---|---|---|---|
| 1 | [`deep-dive-module-1-advanced-traits`](deep-dive-module-1-advanced-traits) | Associated types, default type parameters, trait bounds, newtypes, supertraits | `TaskId` / `TaskTitle` / `TaskDescription` newtypes; `Displayable`, `Storable`, `Validatable` traits |
| 2 | [`deep-dive-module-2-memory-management`](deep-dive-module-2-memory-management) | `Box`, `Rc`, `RefCell`, lifetimes, variance | `GET /tasks/recent` backed by an in-memory recent-tasks cache |
| 3 | [`deep-dive-module-3-unsafe-rust`](deep-dive-module-3-unsafe-rust) | Raw pointers, `unsafe`, C FFI, `macro_rules!` and derive macros | `ETag` header on `GET /tasks/{id}`, computed by a C function (`native/native.c`, linked by `build.rs`) |
| 4 | [`deep-dive-module-4-advanced-concurrency`](deep-dive-module-4-advanced-concurrency) | Threads, channels, `Arc<Mutex<T>>`, `Send`/`Sync`, Tokio tasks and timeouts | `GET /tasks/audit` (a background thread writes the log), an async event consumer, a 3-second query timeout that returns `504` |
| 5 | [`deep-dive-module-5-advanced-web-development`](deep-dive-module-5-advanced-web-development) | Actix middleware, CORS, `anyhow::Context` | `X-Response-Time-Ms` header, CORS, clearer error messages when startup fails |
| 6 | [`deep-dive-module-6-authentication`](deep-dive-module-6-authentication) | `argon2` password hashing, JWTs, auth middleware | `POST /register`, `POST /login`, bearer-token auth on `/tasks`, tasks scoped to each user |
| 7 | [`deep-dive-module-7-websockets`](deep-dive-module-7-websockets) | `actix-ws`, `tokio::sync::broadcast`, Leptos (WASM) | `GET /ws` live task feed, a `shared/` crate for the wire types, a `frontend/` Leptos app |
| 8 | [`deep-dive-module-8-testing-deployment`](deep-dive-module-8-testing-deployment) | Health checks, Docker, CI | `GET /health` (readiness), production Dockerfile and Compose setup, GitHub Actions workflow |

## Layout of a module

```text
deep-dive-module-N-<topic>/
├── Cargo.toml / Cargo.lock
├── README.md            # what the module adds and how to run it
├── RECORDING_GUIDE.md   # recording script for the module's lessons
├── config/default.toml  # app configuration (can be overridden with APP_* env vars)
├── migrations/          # SQLx migrations, run automatically when the app starts
├── src/
│   ├── examples/        # standalone lesson samples, one file per module
│   │                    # (a single examples.rs in Module 1)
│   ├── models/ routes/ db/
│   └── ...              # topic-specific files, e.g. ffi.rs, audit.rs, auth.rs
├── tests/               # testcontainers-backed integration tests
├── native/native.c      # Modules 3+: C library used over FFI
├── build.rs             # Modules 3+: compiles native.c
├── shared/ frontend/    # Modules 7+: wire-type crate and Leptos WASM app
├── Dockerfile
└── docker-compose.yml
```

## Prerequisites

- Rust (stable, edition 2021)
- [OrbStack](https://orbstack.dev) or Docker, for Postgres and the
  integration tests
- Modules 3 and later: a C compiler on `PATH` (`cc`, `gcc` or `clang`)
  - macOS: `xcode-select --install`
  - Debian/Ubuntu: `apt install build-essential`
- Modules 7 and later, frontend only:
  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install trunk
  ```

## Quick start

```sh
cd deep-dive-module-1-advanced-traits
docker compose up -d db   # start Postgres
cargo run                 # API on http://127.0.0.1:8080
```

Run the tests from inside a module folder:

```sh
cargo test --lib   # lesson samples and unit tests, no database needed
cargo test         # full suite; starts a throwaway Postgres container per test
```

For Modules 7 and 8, start the frontend from a second terminal while the
backend is running:

```sh
cd frontend
trunk serve --port 9000 --open
```

The module's own README has the endpoint table, `curl` examples, and a list
of where each lesson's sample code lives.

## Notes

- Module 1 builds on `module-9-final-project` from the earlier course.
  That project isn't in this repository, so the links to it in the module
  READMEs and recording guides don't work here.
- Module 8's CI workflow is at
  `deep-dive-module-8-testing-deployment/.github/workflows/ci.yml`. The course
  treats each module as its own standalone repository. GitHub Actions only
  reads workflows from `.github/workflows/` at the repository root, so the
  workflow won't run in this combined repo until you move it there.
