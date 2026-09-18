# `todo-ws-frontend` — Module 7's Leptos frontend

A Leptos **client-side-rendered (CSR)** single-page app: the browser half of
Module 7. It logs in against the existing backend, lists tasks, creates them,
and — the point of the module — subscribes to the `/ws` broadcast so a task
created by *any* client appears here live.

It shares the exact wire types the backend serializes via the `todo-shared`
crate (`../shared`), so there is a single definition of `Task` for both ends
of the connection. A drift between the two is caught by the backend's
`tests/contract_test.rs`, not at runtime in the browser.

## Prerequisites

The WASM toolchain (installed once):

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Running

Two processes. **First, the backend** (from the module root, one level up):

```sh
cd ..
docker compose up -d db
cargo run
```

**Then this frontend**, in a second terminal:

```sh
trunk serve --open
```

`trunk serve` builds the WASM bundle, serves it (default
<http://127.0.0.1:8080>… note: if the backend already holds that port, pass
`trunk serve --port 9000`), and live-reloads on edits.

Register (or log in) with `alice` / `password123` — see **Registering &
logging in** in the [module README](../README.md#registering--logging-in). A
fresh database has no users, so register one first (a `POST /register` returns
a token and logs you straight in). Open the page in two browser windows and
create a task in one: it appears in both immediately, over `/ws`.

## Why CSR and not SSR?

CSR keeps the backend exactly as Modules 1–6 built it — a JSON + WebSocket
API — and needs no `cargo-leptos` build coordination. It's the closest Leptos
analogue to Blazor WebAssembly. The full-SSR path (`leptos_actix` + server
functions) is a deliberately separate, later step; see the module's `mod-7.md`.
