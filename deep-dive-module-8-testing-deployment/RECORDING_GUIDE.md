# Recording Guide -- Module 8: Testing and Deployment

Target runtime: **~80 minutes**, structured as **12 short, single-topic
lessons** across three parts. Each lesson covers exactly one idea and is meant
to stand on its own -- record them as separate takes, cut them as separate
clips. The three "Part" headers map to `mod-8.md`'s lessons 8.1 / 8.2 / 8.3;
every lesson
carries a **Maps to** line pointing at the matching sample.

Most of this audience is coming from **.NET/C#** -- lessons with a clean .NET
analogue carry a `.NET parallel` line, and they're strong here (`AddHealthChecks`
/`AddNpgSql`, Kubernetes probes, `sdk`→`aspnet` multi-stage builds, `dotnet
test` in Actions). Two lessons are worth over-preparing for (marked ★ ANCHOR):
**L3** (testing the *failure* path) and **L5** (a health check in a slim image
needs a client) -- both are real constraints this module's own code is shaped
around, not hypotheticals.

The framing to keep in mind throughout: this module **does not re-teach
integration testing**. This app has had a real `testcontainers` suite since
Module 1. Say that out loud early -- the temptation (and every generic version
of this lesson) is to introduce a test database here from scratch, which would
be a regression, not progress.

Three things open before recording:
- This folder in the editor, tabs: `src/routes/health.rs`,
  `tests/health_test.rs`, `Dockerfile`, `docker-compose.yml`,
  `.github/workflows/ci.yml`, this `README.md`.
- A terminal here, plus `deep-dive-module-7-websockets` in another window --
  this is a direct continuation, not a new app.
- `docker compose ps` ready to show the `app` service flipping to `healthy`.

Pre-recording checklist:
- [ ] C compiler on `PATH` (`cc --version`) -- Module 3's FFI is still here.
- [ ] Docker/OrbStack running (`docker ps`).
- [ ] `rustup target add wasm32-unknown-unknown` + `trunk` (for the frontend CI
      lesson's context).
- [ ] `cargo test --lib` passes (fast, no DB).
- [ ] `cargo test` passes (needs Docker) -- note `health_test` is slow (~30s)
      because it waits out the pool's connect timeout after killing the
      container; don't let that surprise you on camera.
- [ ] `docker compose up --build` succeeds once off-camera (first image build +
      image pulls are slow).
- [ ] A user seeded via `curl -X POST /register` for the `/health` +
      `docker compose stop db` demo (any user; `/health` doesn't care, but you
      need the app running against a real DB first).

---

## Cold open (~2 min)

Finish Module 7's closing sentence -- it ended: *"seven modules in, this app is
real-time and full-stack. What it isn't yet is *shipped* -- tested end to end
and packaged to run somewhere that isn't your laptop. That's next."* Land the
thesis: *"today, the last mile. Three things: a health check that tells an
orchestrator whether this instance can actually serve a request -- not just
whether the process is breathing. A Dockerfile that matches *this* app, not a
template. And the CI that runs everything we've built on every push. And one
thing we're deliberately *not* doing: re-teaching integration testing. This app
has had a real, container-backed test suite since Module 1 -- we're going to
lean on it, not rebuild it."*

---

## Part 1 -- A Real Health Check (~18 min)

### Lesson 1 -- Liveness vs. readiness (~5 min)
**Maps to:** 8.1 #1 · **File:** none (whiteboard / `mod-8.md`)
- Read the "like you're 5" line: liveness is checking a chest is moving;
  readiness is asking a question and waiting for a sensible answer.
- The distinction, said plainly: *"liveness answers 'should this be
  restarted?' -- keep it cheap, no dependencies, or a slow database gets a
  fine process killed. Readiness answers 'should traffic go here right now?' --
  *that's* the one that checks the database. We're building a readiness check,
  because that's what the Compose healthcheck in Part 2 actually needs."*
- `.NET parallel`: *"`AddHealthChecks()` vs. the same with `.AddNpgSql(...)`;
  and Kubernetes' `livenessProbe` vs. `readinessProbe` -- checked differently,
  used for different decisions: restart the container vs. stop routing to it."*

### Lesson 2 -- The `/health` endpoint (~6 min)
**Maps to:** 8.1 #2 · **File:** `src/routes/health.rs`, `main.rs`
- Walk `health`: `SELECT 1` against the pool, `200` on `Ok`, `503` on `Err`.
  *"`SELECT 1` is the cheapest real query -- it proves a connection can be
  acquired and a round trip completes, without touching a table."*
- Three deliberate choices to call out: **`503`, not `500`** (the app isn't
  broken, a dependency is down -- same reasoning `AppError::Timeout` used
  `504`); **plain `HttpResponse`, not `Result<_, AppError>`** (a health check
  that needs the DB up just to *report* the DB is down is a strange failure
  mode); and **registered outside `require_auth`** (a load balancer polling it
  shouldn't carry a bearer token).

### Lesson 3 ★ ANCHOR -- Testing the *failure* path (~7 min)
**Maps to:** 8.1 #3 · **File:** `tests/health_test.rs`
- This is the lesson, not a footnote: *"a `/health` you only ever call while
  the database is up could return `200` no matter what it checked, and you'd
  never know. The test that matters is the one that proves it *fails*."*
- Walk the test: start Postgres, assert `200`, then `drop(container)` to stop
  it out from under the pool, then assert `503`. *"`drop` is how
  `testcontainers` stops a container -- we're using it on purpose to simulate a
  real outage."*
- Note the poll-until-503 loop: *"the container takes a moment to actually stop
  after `drop`, so we poll rather than asserting on the first read -- same
  retry shape the audit test uses. It's also why this test is a little slow:
  the pool waits out its connect timeout before giving up."* (This is a natural
  lead-in to the README's fast-fail exercise -- mention it exists.)
- Land it: *"the `200` case alone would pass even for a health check hard-coded
  to always return `200`. The failure path is what earns the test its place."*

---

## Part 2 -- Deployment: Docker & CI (~32 min)

### Lesson 4 -- A Dockerfile that matches this app (~7 min)
**Maps to:** 8.2 #1 · **File:** `Dockerfile`
- Read the "kitchen and dining room" analogy for the multi-stage build: the
  builder stage is the messy kitchen (the whole toolchain, every crate, the C
  compiler); only the finished binary crosses into the runtime stage.
- The three "this app specifically" points: **`COPY native ./native`** (without
  it `build.rs`'s `cc::Build` call fails -- Module 3's FFI); **no `libpq`**
  (sqlx's Postgres driver is pure Rust, so `ca-certificates` is all the app
  needs, despite what many tutorials install); **non-root `appuser`** (not the
  `root` a Dockerfile defaults to).
- One more: *"no `cargo test` anywhere in this file. The suite starts its own
  Postgres containers -- a build stage that needs Docker access to run its own
  tests is a sign the tests are in the wrong place. They go in CI."*
- `.NET parallel`: *"`mcr.microsoft.com/dotnet/sdk` (build) →
  `dotnet/aspnet` (runtime) -- the same two-stage shape."*

### Lesson 5 ★ ANCHOR -- A health check in a slim image needs a client (~4 min)
**Maps to:** 8.2 #1–#2 · **File:** `Dockerfile`, `docker-compose.yml`
- The gotcha, stated directly: *"the Compose healthcheck shells out to `curl`.
  But `debian:bookworm-slim` ships no HTTP client. A Dockerfile that installs
  only `ca-certificates` -- exactly what the 'minimal image' instinct tells you
  to do -- builds an image whose healthcheck can *never* pass."*
- Show the fix: `curl` added to the runtime `apt-get install`, with the comment
  that it's there *only* for the healthcheck. *"The teaching point from Lesson 4
  still holds -- no `libpq`, the app needs nothing but `ca-certificates`. `curl`
  is for the container's benefit, not the app's. Two different questions:
  what does the app need, and what does the healthcheck need."*

### Lesson 6 -- Compose: waiting for *real* readiness (~7 min)
**Maps to:** 8.2 #2 · **File:** `docker-compose.yml`
- Two gotchas, both about "started" vs "ready": **`condition:
  service_healthy`** (not a bare `depends_on: - db`) is what makes `app` wait
  for Postgres's `pg_isready` healthcheck to pass, not just for its container
  process to start -- a classic "works on my machine, flaky in CI" source. And
  the **`app` service's own healthcheck** calls `/health` from Lesson 2 --
  proof that endpoint isn't just for a human to `curl`.
- The quiet trap worth its own beat: **`APP_DATABASE_URL`, not `DATABASE_URL`**.
  *"This app's config loader only reads the `APP_`-prefixed names it's used
  since Module 1. Set plain `DATABASE_URL` here and it's silently ignored -- the
  app falls back to its local default and talks to the wrong database, with no
  error. A silently-ignored env var is a debugging trap, not a crash."*
- Demo: `docker compose up --build`, then `docker compose ps` -- watch `app`
  flip to `healthy` a few seconds after start.

### Lesson 7 -- CI: fmt, clippy, and the real suite (~8 min)
**Maps to:** 8.2 #3 · **File:** `.github/workflows/ci.yml` (`backend` job)
- Walk the `backend` job: `cargo fmt --check`, `cargo clippy --all-targets --
  -D warnings`, `cargo test`.
- The point about the test step: *"this runs the *same* integration suite --
  no 'CI mode,' no mocked database. GitHub's `ubuntu-latest` runners already
  have Docker, so `testcontainers` starts a throwaway Postgres per test exactly
  as it does on your laptop. If it passes locally and fails in CI, that's an
  environment difference worth knowing about -- not a different test."*
- `.NET parallel`: *"the same GitHub Actions runners you'd run `dotnet test` on
  -- just `cargo` instead of `dotnet`. Nothing here is Rust-specific practice."*

### Lesson 8 -- CI: the frontend check and a gated image build (~6 min)
**Maps to:** 8.2 #3 · **File:** `.github/workflows/ci.yml` (`frontend`, `docker`)
- The `frontend` job: `fmt --check` + `cargo check --target
  wasm32-unknown-unknown` -- CI that ignored half the module would be a gap.
- The `docker` job: **`needs: backend`** -- *"a build that compiles but whose
  tests are failing shouldn't produce an image anyone deploys. It builds the
  image to prove it builds; it doesn't push it anywhere -- pushing needs a
  registry and credentials, which is an exercise."*
- One honest caveat to say on camera: *"this workflow lives at the module's own
  root, because each module is framed as its own repo. In the combined course
  repo it won't actually trigger -- GitHub only reads `.github/workflows` at the
  true repository root."*

---

## Part 3 -- Cargo for Real (~25 min)

Open Part 3 by reframing: *"deployment isn't only Docker and CI -- it's also
knowing what your build tool is doing. Everything in this part is already
sitting in this app's `Cargo.toml` files; we're just going to read it out
loud."*

### Lesson 9 -- Release profiles (~6 min)
**Maps to:** 8.3 #1 · **File:** `Cargo.toml` (`[profile.release]`), `Dockerfile`
- Start from the Dockerfile line the app already ships: `cargo build --locked
  --release`. *"What does `--release` actually buy? opt-level 3, no debug
  symbols -- and, the part people miss, no overflow checks and no
  `debug_assert!`s."*
- The gotcha to say slowly: *"that's a behavioural difference, not just speed.
  `a + b` that panics on overflow in a debug test *wraps* silently in release.
  Reach for `checked_add`/`saturating_add` where it matters."*
- Then the new `[profile.release]` block: `lto`, `codegen-units = 1`, `strip`.
  *"These trade build time for a smaller, faster binary -- a good deal for an
  image built once in CI, a bad one for your local edit-compile loop, which is
  why they're opt-in and off the dev build."*
- `.NET parallel`: *"Debug vs. Release build configs -- but Rust's release also
  monomorphises and inlines generics and iterator chains aggressively at compile
  time, where .NET's JIT does that at runtime."*

### Lesson 10 ★ ANCHOR -- Features and how they unify (~7 min)
**Maps to:** 8.3 #2 · **File:** `Cargo.toml`
- Point at the features already here: `sqlx = { features = ["chrono",
  "migrate", "runtime-tokio", "postgres"] }`, and (in `shared/Cargo.toml`)
  `chrono = { default-features = false, ... }`. *"Turning a feature *off* is as
  much a choice as turning one on -- that's how `shared` compiles to wasm."*
- The anchor beat -- the rule that surprises everyone: *"features are additive
  and unify across the *whole* dependency graph. If anything, anywhere, turns a
  feature on, it's on for everyone. You cannot turn off what someone else turned
  on."*
- Land it on the app's real example: *"open the `tokio` in `[dev-dependencies]`
  with the `test-util` feature. It's a *second* `tokio` entry, in dev-deps
  specifically, precisely because of this rule -- a dev-dependency's extra
  features apply only to test builds, so the shipped release binary never
  silently gains `test-util`. That comment has been in the file since Module 4;
  now you know why."*
- `.NET parallel`: *"there isn't a clean one -- and that's the point. `#if` is
  per-project and subtractive; NuGet has no feature unification. Additive,
  whole-graph features are genuinely Rust-specific, and the source of most 'why
  is this feature on?' confusion."*

### Lesson 11 -- Workspaces, and why this app isn't one (~6 min)
**Maps to:** 8.3 #3 · **File:** the three-crate layout
- Show what a workspace root would look like (`[workspace] members = [...]`) and
  what it buys: one `Cargo.lock`, one `target/`, whole-repo `cargo build`,
  `cargo test -p shared`.
- The deliberate non-choice: *"this app is three crates -- backend, `shared`,
  `frontend` -- which is a textbook workspace. It isn't one, on purpose. The
  backend is native; the frontend targets `wasm32`. A plain `cargo build` at a
  workspace root would try to build the WASM crate for the host and fail.
  Keeping them independent, joined by a `path` dep on `shared`, lets each be
  built by the right tool for the right target."*
- `.NET parallel`: *"a workspace is a `.sln` grouping `.csproj`s -- but here the
  'solution' is deliberately *not* grouped, because one project targets a
  different platform."*

### Lesson 12 -- Publishing and `--locked` (~6 min)
**Maps to:** 8.3 #4 · **File:** `Dockerfile` (`--locked`)
- Anchor on the Dockerfile again: `--locked`. *"This means 'build exactly what
  `Cargo.lock` pins, and error if the build would change it.' That's what makes
  the image reproducible -- same inputs, same dependency versions, in CI and on
  your machine."*
- Then the publish path, briefly: `cargo package` to inspect, `cargo publish`
  to crates.io (needs `license`/`description` metadata), semver discipline
  (crates.io is append-only -- you `yank`, never delete), and `cargo install`
  for binaries. Mention `cargo tree` and `cargo audit` as the everyday tools.
- `.NET parallel`: *"`cargo publish` ≈ `dotnet pack` + `nuget push`; `cargo
  install` ≈ `dotnet tool install -g`; `Cargo.lock` + `--locked` ≈
  `packages.lock.json` with locked-mode restore."*

---

## Wrap-up (~3 min)

- One sentence each: a **readiness** check (`503`, not `500`; checks the DB;
  public) tested on its **failure** path; a Dockerfile matched to *this* app
  (C dep, no libpq, non-root, +curl for the healthcheck); Compose that waits
  for real readiness (`service_healthy`, exact `APP_` env names); CI that runs
  the real suite and gates the image on it; and a proper look at Cargo -- the
  release profile the image ships, features and their whole-graph unification,
  and why the app is three crates but not a workspace.
- Name the three anchors once more: test the failure path (L3), the
  slim-image-needs-a-client gotcha (L5), and feature unification (L10).
- Point at the **Exercises** in the README / `mod-8.md` -- especially making
  `/health` fast-fail, and pushing the image to a registry + deploying it.
- Close the course: *"eight modules in, this is a complete, tested, CI-checked,
  real-time full-stack Rust app -- structured logging and config, Postgres with
  migrations and pooling, traits and generics, smart pointers and lifetimes,
  unsafe/FFI, concurrency, middleware, JWT auth, per-user WebSocket
  broadcasting, a Leptos frontend, and a deployment story. From here it's the
  real world: push the image to a registry, deploy it, and watch it in
  production."*

---

## Timing summary

| # | Lesson | Part | Target |
|---|---|---|---|
| — | Cold open | — | 2 min |
| 1 | Liveness vs. readiness | 1 | 5 min |
| 2 | The `/health` endpoint | 1 | 6 min |
| 3 ★ | Testing the *failure* path | 1 | 7 min |
| 4 | A Dockerfile that matches this app | 2 | 7 min |
| 5 ★ | A health check in a slim image needs a client | 2 | 4 min |
| 6 | Compose: waiting for real readiness | 2 | 7 min |
| 7 | CI: fmt, clippy, and the real suite | 2 | 8 min |
| 8 | CI: frontend check + gated image build | 2 | 6 min |
| 9 | Release profiles | 3 | 6 min |
| 10 ★ | Features and how they unify | 3 | 7 min |
| 11 | Workspaces, and why this app isn't one | 3 | 6 min |
| 12 | Publishing and `--locked` | 3 | 6 min |
| — | Wrap-up | — | 3 min |
| | **Total** | | **~80 min** |

Cutting to a tighter cap: fold L1 into a one-line callout ("restart? vs. route
traffic? -- see the README"), merge L8's frontend beat into L7, and compress
L11+L12 into a single "Cargo beyond `--release`" lesson if Part 3 runs long.
Don't cut L3, L5, or L10 -- the three anchors, and the three things a generic
version of this lesson gets wrong (never testing failure; a healthcheck that
can't run in the image; assuming features are local and subtractive).
