# Recording Guide -- Module 7: WebSockets & a Real Frontend

Target runtime: **~95 minutes**, structured as **20 short, single-topic
lessons** across three parts. Each lesson covers exactly one idea and is meant
to stand on its own -- record them as separate takes, cut them as separate
clips. The three "Part" headers below map to `mod-7.md`'s lessons 7.1 / 7.2 /
7.3; every lesson carries a **Maps to** line pointing at the matching sample.

Most of this audience is coming from **.NET/C#** -- lessons that have a clean
.NET analogue carry a `.NET parallel` line. Three lessons are the ones to
over-prepare for (marked ★ ANCHOR): **L8** (subscribe-before-spawn), **L12**
(why the frontend can't reuse `models::Task`), and **L19** (CORS). They're the
highest information-per-minute moments, and each is a real constraint the code
is shaped around, not a hypothetical.

Three things open before recording:
- This folder in the editor, tabs: `src/routes/websocket.rs`,
  `src/broadcast.rs`, `shared/src/lib.rs`, `frontend/src/main.rs`,
  `tests/contract_test.rs`, this `README.md`.
- Two terminals here (backend + `trunk`), plus
  `deep-dive-module-6-authentication` open in another window.
- A browser with **two windows** ready for the L20 live demo.

Pre-recording checklist:
- [ ] C compiler on `PATH` (`cc --version`) -- Module 3's FFI is still here.
- [ ] `rustup target add wasm32-unknown-unknown` and `cargo install trunk`
      done (`trunk --version`).
- [ ] `docker compose up -d db` run once and healthy (`docker compose ps`).
- [ ] `cargo test --lib` passes (no DB; warms the C cache, binds the WS socket).
- [ ] `cargo test --test contract_test` passes (pure serde, no DB).
- [ ] `cargo test` passes (needs Docker/OrbStack) -- the whole flow.
- [ ] `cd frontend && trunk build` succeeds once (first Leptos+WASM compile is
      slow -- do it off-camera).
- [ ] A user to log in with -- `curl -X POST /register` seeds one and returns a
      token (registration was added back in Module 6), so there's no Argon2 hash
      to paste on camera. The frontend has a login form only, so register via
      `curl` first, then log in through the UI in L16/L20.
- [ ] Backend (`cargo run`, 8080) and `trunk serve --port 9000` both up before
      Part 3.
- [ ] Optional: `websocat ws://127.0.0.1:8080/ws` ready for L10.

---

## Cold open (~2 min)

Finish Module 6's closing sentence -- it ended: *"six modules in, this app can
prove who's making a request. It still can't tell you anything in real time --
every response so far has been request-in, response-out. Next module opens a
connection that stays open."* Land the thesis: *"today, two things. First, that
open connection -- and a way to shout one event out to every client watching.
Then the part most courses stop short of: a real browser frontend, in Rust,
that listens. And because it's Rust on both ends, the frontend and backend
share the exact same `Task` -- one definition, compiled into the server and
the browser. Something a JavaScript frontend structurally cannot do."*

---

## Part 1 -- The WebSocket Connection (~23 min)

### Lesson 1 -- What a WebSocket actually is (~4 min)
**Maps to:** 7.1 #1 · **File:** none (whiteboard / `mod-7.md`)
- The postcard-vs-phone-call picture; read the "like you're 5" line slowly.
- One sentence to land: *"every route until now finishes. This one stays open,
  and either side can talk at any time."*
- `.NET parallel`: *"`System.Net.WebSockets.WebSocket` -- a `SendAsync`/
  `ReceiveAsync` loop. Most apps reach for SignalR instead; that's Part 2."*

### Lesson 2 -- Why `actix-ws`, not `actix-web-actors` (~3 min)
**Maps to:** 7.1 #2 · **File:** `Cargo.toml`
- The one strong opinion: *"most Actix WS tutorials use `actix-web-actors` --
  `Actor`/`StreamHandler`/`Addr<T>`. It's marked deprecated on crates.io -- I
  checked the crate's metadata, not a blog post. `actix-ws` is WebSockets
  without actors: an `async fn`, a stream, a `Session`."*
- Note `serde_json` moving from dev-deps to a real dependency (the broadcast
  path serializes a `Task`).

### Lesson 3 -- Accepting a connection: `handle()` (~5 min)
**Maps to:** 7.1 #3 · **File:** `src/routes/websocket.rs`
- Walk the three returns of `actix_ws::handle(&req, body)`: the `HttpResponse`
  (completes the handshake), a `Session` (send *to* this client), a
  `MessageStream` (receive *from* it).
- Land the hand-off: *"`handle` returns immediately -- that's the handshake.
  Everything else runs on a spawned task for the life of the connection, the
  same shape `AuditLogger` uses for its background thread."*

### Lesson 4 -- The connection loop, and never `.unwrap()` a send (~5 min)
**Maps to:** 7.1 #3 · **File:** `src/routes/websocket.rs`
- Walk the `while let Some(Ok(msg))` loop: answer `Ping` with `pong`, handle
  `Close`.
- The single point: *"never `.unwrap()` a send in this loop. The first send
  after a client disconnects returns `Err(Closed)` -- `.unwrap()` there panics
  the task. Breaking on `.is_err()` is the whole defense."*
- `.NET parallel`: *"the `while (State == Open)` receive loop, minus the manual
  buffer juggling."*

### Lesson 5 -- Testing over a real socket (~6 min)
**Maps to:** 7.1 #4 · **File:** `src/routes/websocket.rs` (`tests`)
- The "one place in the course" beat: *"every other test uses
  `test::init_service`, which never opens a socket. A handshake needs one -- so
  this is the only test in the course that reaches for `actix_test::start` and
  `awc`."*
- Run `cargo test --lib routes::websocket`; point at the `awc::ws::Frame` vs
  `actix_ws::Message` mirror -- same protocol, two ends.

---

## Part 2 -- Broadcasting (~27 min)

### Lesson 6 -- A broadcast channel, not a client registry (~5 min)
**Maps to:** 7.2 #1 · **File:** `src/broadcast.rs`
- Name the temptation and refuse it: *"the instinct is
  `Arc<Mutex<Vec<Session>>>`. Don't -- `tokio::sync::broadcast` does the
  fan-out *and* the cleanup."*
- Read the radio-station "like you're 5" line; contrast Module 4's `mpsc`
  mailbox.
- `.NET parallel`: *"SignalR's `Clients.All.SendAsync` in miniature -- one
  call, no connection list you maintain by hand."*

### Lesson 7 -- Racing two sources with `tokio::select!` (~4 min)
**Maps to:** 7.2 #2 · **File:** `src/routes/websocket.rs`
- Show the `select!` loop; name it: *"same macro as Module 4's racing lesson.
  Not a new tool -- a new use. It races 'a frame from this client' against 'an
  update for everyone.'"*
- Keep this lesson to *just* the racing idea; the ordering and error-handling
  are the next two lessons.

### Lesson 8 ★ ANCHOR -- Subscribe *before* you spawn (~6 min)
**Maps to:** 7.2 #2 · **File:** `src/routes/websocket.rs`
- Point at `let mut updates = broadcaster.subscribe();` sitting *above*
  `rt::spawn`, not inside it.
- The whole lesson: *"this runs synchronously, before the handshake response
  goes back -- so the client is a subscriber the moment it's connected. Move it
  *inside* the spawned task and you open a gap: a broadcast fired between
  'handshake done' and 'task scheduled and subscribed' is silently lost. And it
  wouldn't fail a test reliably -- it'd make one flaky. This ordering is what
  lets the L5 broadcast test fire the instant the client connects and *know*
  the subscription exists."*

### Lesson 9 -- `Lagged` is not `Closed` (~4 min)
**Maps to:** 7.2 #2 · **File:** `src/routes/websocket.rs`
- The two `recv()` error arms: *"`Lagged` means this one slow client fell
  behind the channel's capacity -- we `continue`, keep it connected, let it
  miss a few. `Closed` means done. Treating `Lagged` as fatal kicks a
  merely-slow client off for no reason."*

### Lesson 10 -- Broadcasting a created task (~5 min)
**Maps to:** 7.2 #3 · **File:** `src/routes/tasks.rs` (`create_task`)
- Show the new `broadcaster` param and `serde_json::to_string(&task)`. Read the
  comment: *"same `Task` GET /tasks returns -- one shape, two delivery paths.
  Fire-and-forget: a serialize failure logs, it doesn't fail the request,
  because the task really was created."*
- Contrast durability: *"the audit log records whether or not anyone's
  watching. A broadcast is the opposite on purpose -- disconnected, you never
  hear it. Different tool, different problem."*
- Optional demo: `websocat` in one terminal, `curl -X POST /tasks` with a
  bearer token in another; watch the JSON arrive. *"Any WS client works -- the
  browser one in Part 3 is just a nicer front for this."*

### Lesson 11 -- Registering the `Broadcaster` and `/ws` (~3 min)
**Maps to:** 7.2 #3 · **File:** `src/main.rs`
- One `Broadcaster::new()`, registered once as `web::Data` (like
  `RecentTasksCache`/`AuditLogger`), and `/ws` wired at the App level.
- Flag once: *"`/ws` is unprotected here -- there's an exercise to wrap it in
  `require_auth`."*

---

## Part 3 -- The Frontend (~41 min)

Open Part 3 by reframing: *"everything so far ends at 'the server broadcasts.'
A broadcast nobody watches is hard to believe in. Let's build the watcher -- in
Rust, in the browser."*

### Lesson 12 ★ ANCHOR -- Why the frontend can't reuse `models::Task` (~4 min)
**Maps to:** 7.3 #1 · **File:** `src/models/task.rs`
- Open the real `Task`: *"`sqlx::FromRow`, `sqlx::Type`, validation returning
  `AppError` (which pulls in actix), and `etag()` calling into C through our
  FFI sample. None of that compiles to `wasm32`, and none of it means anything
  in a browser."*
- Land the problem this lesson exists to pose: *"so we can't literally share
  this struct. The next lesson is the fix."*

### Lesson 13 -- The `shared` crate: one wire type, both ends (~4 min)
**Maps to:** 7.3 #1 · **File:** `shared/src/lib.rs`, `shared/Cargo.toml`
- Show the flat `Task`; then `Cargo.toml`: *"look at what's *not* here -- no
  sqlx, no actix, no FFI. chrono with `default-features = false` to drop the
  clock, so the WASM build stays lean. Just the JSON both ends exchange."*
- `.NET parallel`: *"the shared C# class library a Blazor app references from
  both the API and the WASM project."*

### Lesson 14 -- The contract test across the boundary (~5 min)
**Maps to:** 7.3 #4 · **File:** `tests/contract_test.rs`
- The drift risk and its fix: *"two definitions can drift. So the backend
  serializes its real `Task` and deserializes it into the shared one, field by
  field -- pure serde, no database. Rename a field on one side, this goes red.
  A runtime browser parse error becomes a failed `cargo test`."*
- Run `cargo test --test contract_test`.
- `.NET parallel`: *"a consumer-driven contract test in spirit -- minus the
  HTTP, because both sides are Rust."*

### Lesson 15 -- Leptos, signals, and components (~5 min)
**Maps to:** 7.3 #2 · **File:** `frontend/src/main.rs`, `frontend/Cargo.toml`
- Frame it: *"Leptos, client-side rendered. Reactive signals, not a virtual-DOM
  diff. If you know React or Blazor, this shape is familiar."*
- Show `RwSignal` being `Copy`: *"I move the same `token` signal into the login
  handler, the create handler, and the view -- no cloning ceremony."*
- `.NET parallel`: *"`RwSignal<T>` is `@bind` + `StateHasChanged`, fine-grained;
  `#[component] fn App()` is a Razor component."*

### Lesson 16 -- Logging in and holding the token (~5 min)
**Maps to:** 7.3 #2 · **File:** `frontend/src/main.rs` (`do_login`)
- Walk `do_login`: POST `/login` with `gloo-net`, stash the token in a signal,
  then load the list and open `/ws`.
- The point: *"the token rides every `/tasks` request as `Authorization:
  Bearer`. Module 6's middleware is on the other end -- this frontend is its
  first real client."*

### Lesson 17 -- The browser end of the WebSocket (~5 min)
**Maps to:** 7.3 #3 · **File:** `frontend/src/main.rs` (`subscribe_ws`)
- Show `WebSocket::open`, `.split()`, the `reader.next().await` loop,
  `tasks.update(|list| list.insert(0, task))`.
- Name the mirror: *"this is Part 2's server handler reflected. There we
  dropped a `broadcast::Receiver` to clean up; here we drop the socket's read
  half. Same idea, other end."*

### Lesson 18 -- Fire-and-forget create: the round-trip payoff (~4 min)
**Maps to:** 7.3 #3 · **File:** `frontend/src/main.rs` (`create_task`)
- Point at `create_task` doing the POST and then *nothing*.
- The payoff beat: *"the 'Add' button doesn't draw the task. It tells the
  server, the server broadcasts it, and the same loop that draws everyone
  else's tasks draws mine when the broadcast comes back. One code path for my
  news and everyone's -- which means if the live feed were broken, my own new
  tasks wouldn't appear either. The round trip is proven every time I use it."*
- Read the "one set of ears, everybody's news" line.

### Lesson 19 ★ ANCHOR -- CORS: the seam between two origins (~5 min)
**Maps to:** 7.3 #5 · **File:** `src/main.rs` (`Cors`)
- The one backend change this whole module makes: *"the frontend is a different
  origin from the API. The browser's `Authorization` and `Content-Type` headers
  only survive the CORS preflight if the backend allows them -- so the dev
  `Cors` gained `.allow_any_header()`. Leave it out and every authed request
  fails at preflight, before any handler runs, with nothing obvious in the
  server log."*
- Show the single added line.

### Lesson 20 -- The two-window live demo (~4 min)
**Maps to:** 7.3 #5 · **File:** the running app
- Backend on 8080, `trunk serve --port 9000`, log in with the seeded user.
- Two browser windows side by side; create a task in one; both update. *"That's
  Part 2 and Part 3 together, in front of you. Nobody polled. Nobody
  refreshed."*

---

## Wrap-up (~3 min)

- One sentence each: `actix-ws` for the connection (not the deprecated actor
  crate), `tokio::sync::broadcast` for fan-out (no registry), `tokio::select!`
  reused from Module 4, a `shared` crate as the single source of truth (guarded
  by a contract test), and Leptos CSR consuming it all.
- Name the three anchors once more: subscribe-before-spawn (L8), the sqlx/WASM
  coupling (L12), and CORS (L19).
- Point at the **Exercises** in the README / `mod-7.md` -- not solved on camera,
  especially protecting `/ws` and the browser-header wrinkle it exposes.
- Tease Module 8, same continuation framing: *"seven modules in, this app is
  real-time and full-stack. What it isn't yet is *shipped* -- tested end to end
  and packaged to run somewhere that isn't your laptop. That's next."*

---

## Timing summary

| # | Lesson | Part | Target |
|---|---|---|---|
| — | Cold open | — | 2 min |
| 1 | What a WebSocket actually is | 1 | 4 min |
| 2 | Why `actix-ws`, not `actix-web-actors` | 1 | 3 min |
| 3 | Accepting a connection: `handle()` | 1 | 5 min |
| 4 | The connection loop, never `.unwrap()` a send | 1 | 5 min |
| 5 | Testing over a real socket | 1 | 6 min |
| 6 | A broadcast channel, not a client registry | 2 | 5 min |
| 7 | Racing two sources with `tokio::select!` | 2 | 4 min |
| 8 ★ | Subscribe *before* you spawn | 2 | 6 min |
| 9 | `Lagged` is not `Closed` | 2 | 4 min |
| 10 | Broadcasting a created task | 2 | 5 min |
| 11 | Registering the `Broadcaster` and `/ws` | 2 | 3 min |
| 12 ★ | Why the frontend can't reuse `models::Task` | 3 | 4 min |
| 13 | The `shared` crate: one wire type, both ends | 3 | 4 min |
| 14 | The contract test across the boundary | 3 | 5 min |
| 15 | Leptos, signals, and components | 3 | 5 min |
| 16 | Logging in and holding the token | 3 | 5 min |
| 17 | The browser end of the WebSocket | 3 | 5 min |
| 18 | Fire-and-forget create: the round-trip payoff | 3 | 4 min |
| 19 ★ | CORS: the seam between two origins | 3 | 5 min |
| 20 | The two-window live demo | 3 | 4 min |
| — | Wrap-up | — | 3 min |
| | **Total** | | **~96 min** |

Cutting to a tighter cap: fold L1 into a one-line callout ("postcard vs. phone
call -- see the README"), and merge L7 into L8 (show the `select!` loop as you
explain the ordering). Don't cut L8, L12, or L19 -- the three anchors.
