# Recording Guide -- Module 5: Advanced Web Development

Target runtime: **~53 minutes** of lesson content (two lessons, 5.1 and
5.2). This runs shorter than a generic Module 5's ~75-minute estimate on
purpose: a typical cohort spends a large chunk of 5.2 actually installing
Postgres and migrating an app off SQLite, and this codebase has been
PostgreSQL-based since early in the course -- there's no live migration to
perform. See the "A note on Lesson 5.2" section of this module's `README.md`
before you record 5.2; say the same thing on camera, don't just skip past
it.

As with Module 4, most of this audience is coming from **.NET/C#** --
every segment below has a ".NET parallel" line. One of them is a genuine
gotcha worth over-preparing for: Actix's middleware ordering runs
*backwards* from ASP.NET Core's, not the same direction. Get segment 5.1.3
right.

Two things should be open before you start recording:

- This folder (`deep-dive-module-5-advanced-web-development`) in the
  editor, with `src/examples/module_5.rs`, `src/middleware.rs`,
  `src/main.rs` and this module's `README.md` in tabs.
- A terminal in this folder, plus
  `deep-dive-module-4-advanced-concurrency` open in a second tab/window --
  this module is a direct continuation, not a new app, same as every module
  has been since Module 2.

Pre-recording checklist:

- [ ] A C compiler is on `PATH` (`cc --version`) -- Module 3's FFI/`ETag`
      feature is still part of this app, so `build.rs` still needs one.
- [ ] `docker compose up -d db` run at least once (image pull is slow the
      first time) and confirmed healthy (`docker compose ps`) before you
      start `cargo run` -- an unhealthy/still-starting container produces a
      real, correctly-formatted startup error (see segment 5.2.6) that will
      confuse the room if it happens by accident instead of on cue.
- [ ] `cargo test --lib` passes (fast, no DB) -- also warms the C build
      cache.
- [ ] Have `curl` (or similar) ready for both applied-for-real demos.
- [ ] Know the one file you'll be temporarily editing live:
      `config/default.toml`'s `database.url` (segment 5.2.6). Have the
      original value copied somewhere you can paste it straight back.

---

## Cold open (~2 min)

Open by finishing Module 4's own sentence. That module closed with:
*"next module turns back toward the web layer itself: middleware, and
moving this app's persistence forward -- the parts of a real API that have
nothing to do with the language and everything to do with the shape of the
system around it."* Land the actual thesis: *"today has two, mostly
unrelated halves. The first is Actix-web middleware -- CORS, a custom
middleware you write yourself, and the one thing about it that trips up
almost everyone coming from another web framework: the order you register
middleware in is not the order it runs in, and if you're coming from
ASP.NET Core specifically, it's backwards from what you're used to. The
second half is about error messages -- specifically, the gap between 'the
database connection failed' and 'the database connection failed, and here's
exactly what we were trying to do when it did.'"*

---

## Lesson 5.1 -- Middleware (~20 min)

### 0. Why This Module Doesn't Re-Teach Logging (~1 min)

Say this before any code, so nobody spends the segment waiting for a
`log`/`env_logger` sample that isn't coming: *"a generic version of this
lesson usually starts with a logging middleware. This app already has one
-- `tracing` plus `tracing-actix-web`'s `TracingLogger`, running since
Module 1, with structured JSON output and optional OpenTelemetry export.
Adding `log`/`env_logger` on top would be a regression, not progress, so
we're skipping straight to the parts that are actually new."*

### 1. CORS Middleware (~3 min)
**File:** `src/examples/module_5.rs`, `mod cors_middleware`

- Run `cors_middleware`'s test and read the request it builds: *"a real
  preflight -- an `OPTIONS` request carrying `Origin` and
  `Access-Control-Request-Method` headers, the same thing a browser sends
  automatically before a cross-origin `fetch`. The assertion checks the
  response actually carries `Access-Control-Allow-Origin` -- not that the
  code compiles, that CORS actually did something."*
- `.NET parallel`: *"this is `services.AddCors(...)` plus `app.UseCors(...)`
  -- same idea, same kind of configuration (allowed origins, methods, max
  age for the preflight cache)."*
- Flag the `allow_any_origin()` call honestly: *"fine for a dev course,
  a real API names its actual frontend origin instead -- see this module's
  README exercises."*

### 2. Custom Middleware (~4 min)
**File:** `src/examples/module_5.rs`, `mod custom_middleware`

- Walk `add_custom_header` line by line: *"`from_fn` takes an `async fn`
  with two things: the incoming request, and `next` -- your handle to 'the
  rest of the pipeline.' Code before `next.call(req).await?` runs on the
  way in. Code after it runs on the way out, and `res` there is the actual
  outgoing response -- mutable, in place."*
- `.NET parallel`, and it's a close one: *"this is exactly the shape of an
  inline ASP.NET Core middleware delegate --
  `app.Use(async (context, next) => { /* before */ await next(context); /* after */ })`.
  Same before/next/after structure, same idea of wrapping the rest of the
  pipeline in one function."*
- Point at `res.headers_mut()` vs. `req.extensions_mut()`: *"this is a real
  response header -- a client will see it. `extensions_mut()`, which an
  earlier, broken version of this lesson used by mistake, is a completely
  different thing: an in-process typed map for passing data between your
  own middleware and handlers. It never reaches the wire at all."*

### 3. Middleware Order (~5 min -- the gotcha segment)
**File:** `src/examples/module_5.rs`, `mod middleware_order`

This is the one to slow down for.

- State the rule before showing why it matters: *"Actix's own docs say it
  plainly: if you `wrap()` multiple times, the last occurrence executes
  first. Middleware nests like an onion -- the last `.wrap()` call is the
  outermost layer."*
- Run `middleware_order`'s test and read the assertion out loud before
  explaining it: *"`middleware_a` registered first, `middleware_b`
  registered last. The recorded order is `B before, A before, A after,
  B after`. B, registered last, wrapped A -- ran before it going in, after
  it coming out."*
- **The .NET gotcha, said explicitly and slowly:** *"if you're coming from
  ASP.NET Core, this is backwards from what you're used to, not the same.
  In `Program.cs`, `app.Use(A); app.Use(B);` runs A's request-phase code
  first, then B's -- registration order **is** request execution order,
  reverse order for the response. Actix does the opposite: the **last**
  `.wrap()` you write is the **first** one that runs for a request. Same
  onion shape, opposite reading direction. Get this backwards in a real
  app and you get, for example, an auth check that silently runs *after*
  the logic it was supposed to be guarding."*
- Land why this isn't just trivia: *"this is proven with a real test, not
  asserted in a comment, specifically because it's exactly the kind of
  claim that's easy to get backwards and ship anyway if nothing checks it."*

### 4. Applied for Real: the Middleware Stack in `main.rs` (~7 min -- the anchor segment)
**Files:** `src/main.rs`, `src/middleware.rs`

1. Open `main.rs`'s three `.wrap()` calls and read the comment above them
   out loud before explaining anything -- it already states the rule this
   segment is about to prove: *"read bottom-up for what a request actually
   hits first."*
2. Walk the reasoning in order: *"`Cors` is innermost -- closest to the
   actual routes. `middleware::timing` wraps *around* `Cors`, not inside
   it -- deliberately. `TracingLogger` is outermost, registered last, so it
   wraps everything."*
3. Ask the room to predict before you run anything: *"a CORS preflight
   request never reaches a route handler at all -- `Cors` answers it
   directly. Does it still get an `X-Response-Time-Ms` header? Does
   `TracingLogger` still log it?"* (Answer: yes to both -- that's the
   point of the nesting order.)
4. Run the app (`cargo run`) and prove it:
   ```sh
   curl -i -X OPTIONS http://127.0.0.1:8080/tasks \
     -H "Origin: https://example.com" \
     -H "Access-Control-Request-Method: GET"
   # HTTP/1.1 200 OK
   # access-control-allow-origin: https://example.com
   # x-response-time-ms: 0
   ```
   Point at `x-response-time-ms` on a response that never touched a route
   handler: *"that header only exists because `middleware::timing` wraps
   `Cors`, not the other way around. Flip that one line and this header
   disappears from every preflight response -- silently, no error, just
   gone from your metrics."*
5. Follow with a normal request to show the header on real traffic too:
   ```sh
   curl -i http://127.0.0.1:8080/tasks
   # HTTP/1.1 200 OK
   # x-response-time-ms: 1
   ```

---

## Lesson 5.2 -- Database & Error Handling (~28 min)

### 1. This App Was Already on Postgres (~4 min)
**Files:** `src/db/connection.rs`, `src/db/queries.rs`, `migrations/`, `tests/tasks_test.rs`

- Say this plainly, don't gloss over it: *"a generic version of this lesson
  spends most of its time here -- installing Postgres, writing the first
  connection code, the first query, the first migration. This app did all
  of that back near the start of the course. `db::connection::create_pool`,
  `db::queries::*`, the migration in `migrations/` -- this is Lesson 5.2's
  PostgreSQL content, already applied for real, already tested end-to-end
  in `tests/tasks_test.rs` against a throwaway container. Today's tour is
  short specifically because there's nothing left to build here."*
- Open `db/connection.rs`'s `skip_all` comment on `create_pool`: *"one line
  worth remembering for the next few segments: never log a database
  connection string, it has credentials in it. That's not a hypothetical --
  it's the reason the applied sample later in this lesson is written the
  way it is."*

### 2. PostgreSQL Setup & Connection Pooling (~4 min)
**File:** `src/examples/module_5.rs`, `mod postgresql_setup`, `mod connection_pooling`

- Run both tests and explain `connect_lazy`: *"building a pool doesn't
  have to mean connecting to anything yet. `connect_lazy` builds a real,
  working `Pool` value with no `.await` at all -- the actual network
  connection waits for the pool's first real query. That's what lets these
  tests run with no Docker, no live database, anywhere."*
- Flag the one surprise, since it cost real debugging time while this
  module was being built: *"`connect_lazy` still needs a Tokio *runtime*
  present, even though it never awaits anything itself -- it spawns a
  background maintenance task as part of building the pool. Plain `#[test]`
  has no runtime; that's why these are `#[tokio::test]`."*
- `connection_pooling`'s test: *"`Arc::clone` twice, `Arc::ptr_eq` to prove
  both handles point at the same pool. This is exactly
  `db::DbPool = Arc<PgPool>` -- every request handler in this app shares
  one pool the same way."*
- Contrast with the real app: *"`db::connection::create_pool` connects
  *eagerly* -- `.connect().await`, not `connect_lazy` -- on purpose, so a
  bad URL fails loudly at startup instead of on whatever request happens to
  hit the database first. You'll see that startup failure for real in a
  few minutes."*

### 3. Migrations (~2 min)
**File:** `src/examples/module_5.rs`, `mod migrations`

- Run the test: *"this reads the app's actual migration file off disk and
  checks it -- proof the migration this lesson describes already exists
  and is well-formed, with no database involved at all."*

### 4. Error Context with `anyhow` (~5 min)
**File:** `src/examples/module_5.rs`, `mod error_context_with_anyhow`

- Run both tests. On the second one, read the assertion on `err.chain()`
  out loud: *"`.context(...)` doesn't replace the original error, it wraps
  it. The message you add shows up first, but the original
  `ParseIntError` -- 'invalid digit found in string' -- is still in there,
  still inspectable."*
- `.NET parallel`: *"closest thing in C# is catching an exception and
  rethrowing a new one with the original as `InnerException` --
  `throw new InvalidOperationException("...", ex)`. `anyhow::Context` gets
  you the same 'here's what I was doing, and here's what actually broke'
  chain, without the `try`/`catch` ceremony -- it's one method call chained
  onto a `Result`."*

### 5. Error Propagation Across Layers (~4 min)
**File:** `src/examples/module_5.rs`, `mod error_propagation_across_layers`

- Walk `find_task` -> `handle_get_task`: *"two layers, `?` at each one, no
  `match` anywhere. `find_task` stands in for a data-access function like
  `db::queries::get_task`; `handle_get_task` stands in for a route
  handler."*
- `.NET parallel`, and it's worth stating why there isn't a closer one:
  *"C# doesn't really have an equivalent of `?`, because it doesn't need
  one -- an uncaught exception already propagates up the call stack on its
  own, automatically, with no keyword required at each layer. Rust's
  `Result` is the opposite default: it does **not** propagate on its own.
  `?` is what makes a function stop and hand the error up to its caller --
  and you can see every single point where that handoff happens just by
  grep-ing for `?`, something you can't do for exceptions in C#."*
- Run both tests, land on the second: *"`status_code()` comes back `404`
  with zero `match` statements written anywhere in this sample -- the
  conversion happens once, via `#[from]` on `AppError`, and every caller
  downstream just gets to `?` and move on."*

### 6. Applied for Real: `anyhow` in `main.rs`'s Startup (~9 min -- the anchor segment)
**Files:** `src/main.rs`

1. Open `main.rs`'s signature: *"`async fn main() -> anyhow::Result<()>`,
   not `io::Result<()>` anymore. `anyhow::Result<T>` is a type alias for
   `Result<T, anyhow::Error>` -- an error type built for exactly this job:
   the outer edge of a program, where nothing downstream needs to `match`
   on *which* error happened, it just needs a clear message and a
   non-zero exit."*
2. Walk the three `.context(...)` calls -- config, database, migrations --
   and say what used to be there: *"before this module, all three of these
   collapsed into `.map_err(io::Error::other)` -- the same generic wrapper
   regardless of which step actually failed. You'd know *something* broke
   at startup. You wouldn't know what, without reading source."*
3. Point at the database-connection `.context(...)` specifically and read
   its comment aloud: *"this one's deliberately different from the lesson
   sample you just saw -- it does **not** include the database URL. A
   Postgres connection string has a password baked into it. Interpolating
   it into an error message -- which is exactly what the standalone lesson
   sample does -- is a straightforward way to leak a credential into your
   logs. 'Failed to connect to the database' is enough to act on without
   that risk."*
4. **Live demo.** Open `config/default.toml`, and change the database
   port to something wrong -- `5432` to `5433` is enough (confirmed live:
   this fails after ~5 seconds, the `acquire_timeout` `db::connection::
   create_pool` sets). Run `cargo run`:
   ```text
   Error: failed to connect to the database

   Caused by:
       pool timed out while waiting for an open connection
   ```
   (The exact "Caused by" line can vary slightly by failure mode -- a
   refused connection, a timeout, an auth failure -- that's `anyhow`
   passing the real underlying `sqlx::Error` straight through underneath
   the added context, not something to worry about matching exactly.)
   Say while it's on screen: *"'failed to connect to the database' -- not
   'failed to load configuration,' not 'failed to run migrations.' That
   distinction is the entire point of this lesson. And notice what's *not*
   here: no `postgres://postgres:postgres@...` connection string, anywhere
   in this output."* **Revert `config/default.toml`'s port back to `5432`
   and confirm `cargo run` succeeds again before moving on.**

---

## Wrap-up (~3 min)

- Recap in one sentence each: `from_fn` for writing a middleware without
  the `Transform`/`Service` trait ceremony, the onion-nesting rule for
  `.wrap()` (and its ASP.NET Core-reversed reading direction), CORS as the
  one real, applied use of it in this app, `anyhow::Context` for startup
  errors that name what actually failed, and `?` for propagating a typed
  application error across layers without a `match` at each hop.
- Point at the **Exercises** section of this module's `README.md` --
  explicitly say you're not solving them on camera.
- Tease Module 6 with the same continuation framing used to open every
  module so far: *"five modules in, and this app has gone from a single
  `main.rs` to something with structured logging, a real database, a
  background worker, request timeouts, CORS, and startup errors you can
  actually act on -- everything a real API needs except one thing: proving
  who's making the request in the first place. Next module is
  authentication."*

---

## Timing summary

| Segment | Target |
|---|---|
| Cold open | 2 min |
| 5.1.0 Why This Module Doesn't Re-Teach Logging | 1 min |
| 5.1.1 CORS Middleware | 3 min |
| 5.1.2 Custom Middleware | 4 min |
| 5.1.3 Middleware Order (the .NET gotcha) | 5 min |
| 5.1.4 Applied for Real (Middleware Stack) | 7 min |
| **Lesson 5.1 subtotal** | **20 min** |
| 5.2.1 This App Was Already on Postgres | 4 min |
| 5.2.2 PostgreSQL Setup & Connection Pooling | 4 min |
| 5.2.3 Migrations | 2 min |
| 5.2.4 Error Context with `anyhow` | 5 min |
| 5.2.5 Error Propagation Across Layers | 4 min |
| 5.2.6 Applied for Real (Startup Errors, live demo) | 9 min |
| **Lesson 5.2 subtotal** | **28 min** |
| Wrap-up | 3 min |
| **Total** | **~53 min** |

If you're cutting to fit a tighter cap, cut 5.2.1's tour to a one-line
callout first ("this app already has Postgres -- see the README"), then
cut 5.1.2 to a mention without running the sample. Don't cut 5.1.3's
ordering explanation or either applied-for-real segment (5.1.4, 5.2.6) --
they're the highest information-per-minute moments in the recording, the
same role Module 3's `ETag` `curl` walkthrough and Module 4's timeout demo
played.
