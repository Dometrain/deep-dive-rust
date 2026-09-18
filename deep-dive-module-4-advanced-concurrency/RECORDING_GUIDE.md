# Recording Guide -- Module 4: Advanced Concurrency

Target runtime: **~100 minutes** of lesson content (a short closures primer,
4.0, then two lessons, 4.1 and 4.2).
Timings are per-segment targets, not hard limits -- if a segment runs long,
cut from "extra context" first, never from the live code.

Most of this audience is coming from **.NET/C#**, not from a systems
language. Every segment below has a spoken ".NET parallel" line built in --
don't skip those even if they feel obvious; they're doing real work, not
just flavor. Where a concept genuinely has no clean .NET equivalent (mainly
`Send`/`Sync`), say so explicitly rather than forcing a comparison that
doesn't hold.

Two things should be open before you start recording:

- This folder (`deep-dive-module-4-advanced-concurrency`) in the editor,
  with `src/examples/module_4.rs`, `src/audit.rs`, `src/consumer.rs`,
  `src/main.rs` and `src/routes/tasks.rs` in tabs.
- A terminal in this folder, plus `deep-dive-module-3-unsafe-rust` open in
  a second tab/window -- this module is a direct continuation, not a new
  app, same as every module has been since Module 2.

Pre-recording checklist:

- [ ] A C compiler is on `PATH` (`cc --version`) -- Module 3's FFI/`ETag`
      feature is still part of this app, so `build.rs` still needs one.
- [ ] `docker compose up -d db` run at least once (image pull is slow the
      first time).
- [ ] `cargo test --lib` passes (fast, no DB) -- also warms the C build
      cache.
- [ ] `cargo test --doc` passes -- this is where the `Send`/`Sync` live
      demo's `compile_fail` block is actually verified; confirm it's green
      before you're rolling, so you're not debugging it on camera.
- [ ] Have `curl` (or similar) ready for all three applied-for-real demos
      (segments 4.1.8, 4.2.8 and 4.2.9).
- [ ] Know the one number you'll be temporarily editing live:
      `routes::tasks::GET_TASK_TIMEOUT` (segment 4.2.8) and nothing else --
      segment 4.1.6's live demo only runs `cargo test --doc`, and 4.2.9
      only runs `cargo run` and reads logs, neither edits a file.

---

## Cold open (~2 min)

Open by finishing Module 3's own sentence. That module closed with:
*"three modules in, and the one thing every single lesson has shared is
that Rust makes you handle a failure mode explicitly, somewhere, instead of
letting it happen silently."* Land the new thesis: *"today's failure mode is
the one every other mainstream language hands you at runtime, in
production, under load: two threads touching the same data at the same
time. In C#, nothing stops you from sharing a `List<T>` between two threads
and mutating it from both -- it compiles, it might even work most of the
time, and then it doesn't, usually at 2am. Rust's compiler refuses to
compile that program at all. Today is about two things: the APIs you use to
write concurrent code, and -- more importantly -- the mechanism that lets
the compiler make that refusal in the first place."*

Frame the shape of the ~100 minutes: *"first a short primer on closures --
the anonymous functions every `spawn` in this module takes -- because the one
keyword that trips people up there (`move`) is the same keyword you'll type in
front of almost every thread and task today. Then two lessons. 4.1 is native
threads: spawn, join, channels, and then the part most 'intro to threads'
content skips -- sharing state directly, safely, and the two compiler-checked
traits that make it safe. 4.2 is the same shapes again, under Tokio, for
I/O-bound work instead of CPU-bound work, plus one new tool for racing two
things against each other."*

---

## Lesson 4.0 -- Closures: The Building Block Behind Every Thread (~15 min)

Every `thread::spawn`/`tokio::spawn` below hands `spawn` a **closure**. Spend a
few minutes on closures themselves first -- especially `move`, which is about to
appear in front of nearly every spawn, and which learners will otherwise treat
as a magic word they copy without understanding.

### 1. Capturing the Environment, and the Three Traits (~8 min)
**File:** `src/examples/module_4.rs`, `mod closures` (live-code the same shapes
on a scratch file if you prefer, but the tested sample is the source of truth).

- Say what a closure *is* before any code: *"an anonymous function that can also
  remember values from the code around it -- capture its environment."* Read the
  "like you're 5" note from `mod-4.md`: a to-do note that also remembers things
  from the room it was written in.
- Open `mod closures` and walk its three functions: `multiplier` (reads its
  capture -> `Fn`, returned as `impl Fn`, callable repeatedly), `run_twice`
  (takes `mut f: impl FnMut()` -> mutates across calls), and `into_greeting`
  (a `move` closure that owns its capture -> `FnOnce`). Each maps to one of the
  three tests below.
- Live-code the three captures, naming the trait each one gets:
  ```rust
  let factor = 3;
  let multiply = |x: i32| x * factor;        // reads -> Fn
  let mut total = 0;
  let mut add = |x: i32| total += x;         // mutates -> FnMut (note `mut` binding)
  let name = String::from("alice");
  let greet = move || format!("hi, {name}"); // owns -> FnOnce
  ```
- The one sentence that unlocks it: *"the compiler picks the trait -- you never
  declare it. `Fn` borrows, `FnMut` borrows mutably, `FnOnce` takes ownership.
  The three names just describe **how the closure touches what it captured**."*
- Flag the common trip-up on camera: the `FnMut` example needs `let mut add`,
  not just a `mut` on the captured variable -- it's the *closure binding* that
  must be `mut`.
- **Coming from .NET:** *"this is a C# lambda -- `x => x * factor`. The
  difference: C# always captures by reference to a hidden object; Rust makes you
  choose (`move` or not) and encodes 'how it's used' as a trait the compiler
  checks. `impl Fn(i32) -> i32` as a parameter is a `Func<int,int>`."*
- Run `cargo test --lib examples::module_4::closures` -- three green tests, one
  per trait -- before moving on.

### 2. `move`, and Why Every Thread Needs It (~7 min)
**File:** `mod closures`'s `into_greeting` (the `move` case); then forward-point at 4.1's spawns.

- This is the bridge into the whole module -- give it the weight: *"watch for
  this exact keyword in every `spawn` for the next 80 minutes."* `into_greeting`
  in `mod closures` is the same shape; a thread makes it concrete:
  ```rust
  let message = String::from("owned by the thread now");
  let handle = std::thread::spawn(move || println!("{message}"));
  handle.join().unwrap();
  // `message` can't be used here -- it moved into the thread.
  ```
- Explain *why the borrow checker demands it*: *"the spawned thread may outlive
  `main`'s stack frame. Without `move`, the closure only borrows `message` --
  and the compiler can't prove `message` will still be alive when the thread
  runs, so it rejects the program. `move` transfers ownership into the closure,
  so the thread owns what it needs and nothing can be freed out from under it."*
- Land the payoff for the rest of the module: *"so when you see
  `thread::spawn(move || ...)` and `tokio::spawn(async move { ... })` in every
  segment coming up, you'll know the `move` isn't boilerplate -- it's the thing
  making the borrow checker accept spawned work that outlives its creator."*
- **Coming from .NET:** *"C# never needs a `move` keyword because the GC keeps
  captured objects alive as long as any thread references them. Rust has no GC,
  so it must *prove* the captures outlive the thread -- `move` is how you give it
  that proof."*

---

## Lesson 4.1 -- Threads, Channels & Shared State (~38 min)

### 0. Threads vs. Async, and What Channels Don't Solve (~2 min)

*"Quick rule of thumb before any code: threads for CPU-bound work, async for
I/O-bound work -- we'll justify that split properly in 4.2. Everything in
this lesson uses real OS threads. And channels solve one shape of problem --
'pass a message from one place to another' -- but not every problem is that
shape. Sometimes several threads genuinely need to read and write the
*same* piece of data. That's the second half of this lesson, and it's the
half most people mean when they say 'shared-state concurrency.'"*

### 1. Spawning Threads (~3 min)
**File:** `src/examples/module_4.rs`, `mod spawning_threads`

- Run `spawn_and_get_result` and walk it: *"`thread::spawn` takes a
  closure, runs it on a new OS thread, and hands you back a `JoinHandle`.
  Coming from .NET, this is closest to `new Thread(() => {...}).Start()` --
  a real thread, not a pooled one. If you're used to `Task.Run`, that
  schedules onto the .NET thread pool instead; Rust's equivalent of that is
  a separate crate, `rayon`, not `std::thread` -- we're staying at the
  lower level today on purpose, because it's what makes the next few
  segments visible."*
- Point at `.join().unwrap()`: *"`join()` blocks until the thread finishes
  and hands back whatever the closure returned, wrapped in a `Result` --
  `Err` only if the thread panicked."*

### 2. Thread Joining (~3 min)
**File:** `src/examples/module_4.rs`, `mod thread_joining`

- Run `spawn_five_and_collect`. Say: *"same idea, five times over, collected
  into a `Vec` in the order they were spawned -- not the order they
  finished. `.join()` on `Vec`'s `handle[i]` waits for *that specific*
  thread, so results always come back in push order even though the
  threads themselves might finish in any order underneath. `join` here is
  literally the same word as `thread.Join()` in C#, doing the same job."*
- Land the stakes: *"if you never call `join`, the spawned thread keeps
  running after the function that spawned it returns -- and if that
  function is `main`, the whole process can exit before the thread finishes
  at all. That's the first row of this module's pitfalls table, and you'll
  see it matter for real in segment 8."*

### 3. Channel Basics (~3 min)
**File:** `src/examples/module_4.rs`, `mod channel_basics`

- Run `send_two_and_collect`. Say: *"a channel is a pneumatic tube -- one
  end sends, the other end receives, and the sender never has to know who's
  listening. `mpsc::channel()` -- multi-producer, single-consumer.
  Closest .NET match is `BlockingCollection<T>`: `Add`/`Take` instead of
  `send`/`recv`, same 'one or more producers, one consumer' shape."*
- Point at `for received in receiver`: *"iterating a receiver blocks,
  waiting for the next message, and the loop only ends once the channel
  closes -- which is exactly what segment 4 is about."*

### 4. Channel Iteration (~3 min)
**File:** `src/examples/module_4.rs`, `mod channel_iteration`

- Run `fan_in_five_messages`. Walk the `.clone()` / `drop(sender)` pair
  carefully: *"five threads, each with its own clone of the sender. The
  *original* sender is dropped right here, on the main thread, once every
  clone has been handed out. The receiver's loop doesn't end when the
  original goes away -- it ends when **every** sender, original and every
  clone, is gone. Forget to drop even one clone somewhere in a real program,
  and this loop just... waits forever. That's this module's 'channel never
  closes' pitfall, and it's the single most common channel bug in practice
  -- much more common than anything exotic."*
- Note the `.sort()` before asserting in the test: *"five threads racing to
  send means no guaranteed arrival order -- sorting before comparing is
  what keeps this test from being flaky, not a code smell."*

### 5. Shared State with `Arc<Mutex<T>>` (~5 min)
**File:** `src/examples/module_4.rs`, `mod shared_state_with_arc_mutex`

- Run `increment_from_ten_threads` and get the 1000 out loud before
  explaining anything: *"ten threads, each incrementing a shared counter a
  hundred times, and the answer comes out exactly right, every single
  time you run it. That's the claim to make good on."*
- Whiteboard analogy: *"one whiteboard, five kids want to write on it. Two
  writing at once is a scribbled mess. A `Mutex` is a marker only one kid
  can hold -- you must be holding it to write, there's only one marker, so
  writes never overlap."*
- `.NET parallel`, said explicitly: *"you'd write this in C# with
  `lock (obj) { ... }`. The important difference: `lock` protects a **block
  of code** -- nothing stops some other part of the codebase from touching
  that shared field *without* going through the lock, and the compiler
  won't catch that mistake. `Mutex<T>` in Rust wraps the **data itself**.
  There is no way to reach the `i32` inside without calling `.lock()`
  first. The discipline C#'s `lock` only asks you to follow, Rust's
  compiler enforces."*
- Point at `Arc::clone` and the lock's release: *"`Arc` answers a different
  question than `Mutex` -- 'who's allowed to free this once nobody needs
  it,' since Rust has no garbage collector doing that for you. And the lock
  releases the instant `num` goes out of scope at the end of that inner
  block -- no `unlock()` to forget."*

### 6. Why the Compiler Won't Let You Cheat: `Send` and `Sync` (~6 min -- live demo)
**File:** `src/examples/module_4.rs`, `mod send_and_sync`

This is the thesis segment for the whole module -- protect it on time.

- State the two definitions plainly, then say there's no real .NET
  equivalent: *"`Send` means 'safe to move to another thread.' `Sync` means
  'safe to access from multiple threads at once through a shared
  reference.' C# has nothing that checks this at compile time -- passing a
  plain, non-thread-safe `List<T>` into a second thread compiles exactly as
  fine as passing a `ConcurrentBag<T>`. The compiler can't tell the
  difference. The mistake shows up later, at runtime, as a corrupted
  collection -- if you're lucky enough to catch it at all."*
- Run `share_via_arc_across_a_thread` first, the working case: *"`Arc`'s
  reference count is updated atomically, so `Arc<T>` is `Send` -- this
  compiles and runs."*
- **Live demo -- run `cargo test --doc examples::module_4::send_and_sync`.**
  Point at the doc comment's `compile_fail` block before running it: *"this
  is the same idea with `Rc` instead of `Arc` -- `Rc`'s reference count is a
  plain, non-atomic integer. `rustdoc` actually tries to compile this block
  and fails the test if it *doesn't* fail to compile -- so this isn't a
  claim in a comment that could go stale, it's checked, every time this
  test suite runs."* Run it, show the green `compile fail ... ok` line,
  then say the payoff: *"there's no test to write for the mistake version,
  no code review comment to leave. It doesn't build. That is the entire
  mechanism behind the phrase 'fearless concurrency' -- not carefulness, not
  discipline. A category of bug that most languages catch (if you're
  lucky) in production, Rust's compiler refuses to let exist."*

### 7. Borrowing Data Safely with Scoped Threads (~4 min)
**File:** `src/examples/module_4.rs`, `mod scoped_threads`

- Run `sum_chunks` and immediately point at what's missing: *"no `Arc`
  anywhere. `thread::scope` guarantees every thread spawned inside it
  finishes before the call returns, so the compiler can prove a plain
  borrow of `numbers` stays valid for the threads' whole lifetime -- no
  need to promise the data lives forever."*
- `.NET parallel`: *".NET never needed this feature at all, and it's worth
  saying why: the garbage collector already keeps any object alive for as
  long as *any* thread might still reference it. 'The data might get freed
  before the thread finishes' isn't a problem C# can have. Rust needs
  `thread::scope` precisely *because* it has no garbage collector -- without
  it, borrowing a short-lived local across threads would force you into
  `Arc` even for a case this simple."*
- Land the rule of thumb: *"reach for `scope` first when threads are
  short-lived and don't need to outlive the current function. Reach for
  `Arc<Mutex<T>>`, from segment 5, when the data genuinely needs to be
  shared beyond that."*

### 8. Applied for Real: the Background Audit Trail (~7 min -- the anchor segment)
**Files:** `src/audit.rs`, `src/main.rs`, `src/routes/tasks.rs`

Same role Module 3's `ETag` segment played -- protect this on time.

1. Open `README.md`'s "why concurrency primitives stay out of most of
   `src/routes`" section and read the first paragraph aloud: *"a single
   `.await` on a single database call has no reason to spawn a thread.
   That's still true for three of the four CRUD handlers. This segment is
   the one place in the whole app that earns a real background thread."*
2. Open `src/audit.rs` top to bottom: *"`AuditLogger::spawn` starts exactly
   one background thread and returns two things -- `AuditLogger`, cheap to
   clone into every Actix worker, and `AuditWorker`, kept exactly once.
   That asymmetry -- many loggers, one worker -- is what makes 'join this on
   shutdown' a single, unambiguous call site instead of something every
   clone has to coordinate."* Point at `record`: *"this is a channel send.
   Not a lock, not a database write, not anything that could make a request
   slower. The actual work -- appending to the `Mutex`-guarded log, and
   logging it -- happens over on the background thread, off the request
   path entirely."*
3. Open `src/routes/tasks.rs`'s `create_task` and point at the one new
   line: `audit.record(task.id, ...)`. Then open `src/main.rs` and trace
   the full lifecycle out loud: *"spawned once, here. Cloned into every
   worker, there. And after `.run().await?` -- after Actix has already torn
   down every worker and, with it, every clone of the sender -- `
   audit_worker.shutdown()` joins the thread. Read the comment on that
   line: this call only returns promptly *because* every sender is already
   gone by the time we reach it. Call it while a logger's still alive
   somewhere, and it hangs forever -- the exact 'forgot to drop a sender'
   pitfall from segment 4, now with real stakes."*
4. Run the app (`cargo run`), then:
   ```sh
   curl -s -X POST http://127.0.0.1:8080/tasks \
     -H 'content-type: application/json' \
     -d '{"title": "Learn concurrent Rust"}'

   curl -s http://127.0.0.1:8080/tasks/audit
   ```
   Point at the response: *"one event, recorded by a thread that request
   handler never waited on. This is the entire point of offloading work
   like this: the response came back the instant the database insert
   finished -- the audit write never sat on the critical path at all."*

---

## Lesson 4.2 -- Async/Await with Tokio (~31 min)

### 1. Tokio Runtime (~2 min)
**File:** `src/examples/module_4.rs`, `mod tokio_runtime`

- *"An `async fn` doesn't run itself -- it's a recipe card that says 'wait
  for this, then do the next step.' Something has to actually watch and
  come back. Tokio is that something: a scheduler juggling thousands of
  'waiting on something' tasks across a handful of real threads."*
- `.NET parallel`: *"this is exactly the thread pool that powers `async`/
  `await` and `Task` in .NET -- you don't think about it because the CLR
  sets it up for you implicitly. `#[tokio::main]` is Rust being explicit
  about the same setup step."* Run
  `cargo test --lib examples::module_4::tokio_runtime` and point at
  `Handle::try_current().is_ok()`: *"there's nothing to demo beyond 'yes, a
  runtime is really driving this test' -- which is exactly what this
  asserts."*

### 2. Spawning Async Tasks (~4 min)
**File:** `src/examples/module_4.rs`, `mod spawning_async_tasks`

- Run `run_two_concurrently`. *".NET parallel: `tokio::spawn(async {...})`
  is `Task.Run(async () => {...})` -- schedules the block to run
  concurrently, hands back a handle to await."* Point at the timing
  assertion: *"one task sleeps 50ms, the other 100ms, and the whole
  function returns in about 100ms, not 150 -- they ran concurrently, the
  same way the two `tokio::spawn`ed threads-that-aren't-threads always do."*
- Point at `#[tokio::test(start_paused = true)]` on the test itself, worth a
  short aside: *"that attribute gives the test Tokio's virtual clock --
  every `sleep` in `run_two_concurrently` still suspends in the right
  order, but time itself jumps straight to the next pending timer instead
  of actually waiting. That's why the assertion below can check for
  *exactly* 100 milliseconds elapsed, not 'somewhere under 140ish' -- no
  real-world CPU jitter to allow for, and the test runs instantly instead
  of taking a tenth of a second. Worth remembering any time you're tempted
  to assert on wall-clock timing in a test."*

### 3. Async Channels (~4 min)
**File:** `src/examples/module_4.rs`, `mod async_channels`

- Run `send_and_receive_five`. *".NET parallel: think `Channel<T>` from
  `System.Threading.Channels`, introduced in .NET Core 3.0 -- not
  `BlockingCollection<T>` from segment 4.1.3. `Channel<T>` is the
  async-native one: awaiting a full channel yields control back to the
  scheduler instead of blocking a real thread, exactly like
  `tokio::sync::mpsc` here."*

### 4. Task Scheduling (~4 min)
**File:** `src/examples/module_4.rs`, `mod task_scheduling`

- Run `schedule_five`, and call out the deliberately-reversed sleep
  durations: *"task 0 sleeps longest, task 4 sleeps least -- so internally
  they finish in the opposite order from how they were pushed. And the
  result still comes back `[0, 1, 2, 3, 4]`. `handle.await` waits for
  *that specific task*, not 'whichever task finishes next' -- same lesson
  as `thread_joining` in 4.1, one level up the stack."*
- `.NET parallel`: *"this whole pattern -- `List<Task<T>>`, push, then
  `Task.WhenAll(tasks)` -- is what you'd reach for in C# for the same
  job."*

### 5. Shared Async State: `tokio::sync::Mutex` vs. `std::sync::Mutex` (~6 min)
**File:** `src/examples/module_4.rs`, `mod shared_async_state`

- Run `increment_from_five_tasks` and get the correct count out loud first,
  same as segment 4.1.5.
- Ask directly: *"why not just reuse the `Mutex` from segment 5?"* Answer
  it precisely: *"`std::sync::Mutex::lock()` is a *blocking* call -- it
  parks the real OS thread until the lock is free. Do that inside an
  `async fn` running on Tokio, and you can freeze the worker thread that
  was supposed to be juggling *other* tasks too -- work that has nothing to
  do with your lock stalls right along with it."*
- `.NET parallel`, said precisely because it's a strong one: *"C#'s `lock`
  keyword can't wrap an `await` at all -- the compiler flatly refuses to
  compile a `lock` block containing one, for exactly this hazard. The
  idiomatic C# fix is `SemaphoreSlim(1, 1)` used as an async lock:
  `await semaphore.WaitAsync()`, then `Release()` in a `finally`.
  `tokio::sync::Mutex` is Rust's version of that same fix -- `.lock()`
  returns a future you `.await`, so waiting for it never blocks the thread
  that's supposed to be running other tasks."*
- State the rule of thumb plainly, it's the takeaway: *"`std::sync::Mutex`
  for threads. `tokio::sync::Mutex` only when the lock might be held across
  an `.await` point inside async code. Reaching for the async one
  everywhere 'just in case' isn't wrong, exactly, but it's paying for a
  guarantee most locks in a Tokio app don't need."*

### 6. Racing Tasks with `tokio::select!` (~5 min)
**File:** `src/examples/module_4.rs`, `mod racing_tasks_with_select`

- Run both tests in `racing_tasks_with_select` back to back -- fast fetch
  wins, then slow fetch times out. *"same function, `fetch_with_timeout`,
  two different outcomes depending purely on which branch resolves first.
  `tokio::select!` runs every branch concurrently and proceeds with
  whichever finishes first -- and here's the word that matters for the next
  ten minutes: it **cancels** the rest. Not 'lets it finish quietly in the
  background' -- cancels. The losing branch's future gets dropped, right
  there, mid-execution."*
- `.NET parallel`: *"closest match is `Task.WhenAny`, but `select!` is a
  language-level macro, not a method you inspect afterward -- each branch
  pattern-matches its own result and runs different code, closer to a
  `switch` over 'whichever of these `await`s resolves first.'"*
- Land the concept by name, deliberately, before moving on: *"'is it safe
  to drop this future mid-flight' has an actual name in the Tokio docs --
  cancellation safety -- and it's worth knowing the term even though
  `fetch_data` here dodges the question entirely: it's a bare `sleep`, no
  side effects, so cancelling it costs nothing. Read the doc comment on
  `fetch_with_timeout` -- it says exactly that, and points at where the
  real answer lives. A channel `recv()` is safe to cancel. A half-sent
  write usually isn't. You don't get to skip asking the question just
  because this toy example didn't need to answer it."* Tease the applied
  segment: *"this exact shape -- race the real work against a timeout --
  is what a real endpoint in this app does for real, on an actual
  database call, in a few minutes. That's where the question gets a real
  answer."*

### 7. Background Async Worker (~4 min)
**File:** `src/examples/module_4.rs`, `mod background_async_worker`

- Run the test and point at what's *not* different from 4.1's
  `background_task_processor` before pointing at what is: *"a channel,
  a worker consuming from it until the sender's dropped, a handle you wait
  on before trusting the result -- same shape as 4.1, same pitfalls
  (forget to drop the sender, this hangs forever; forget to wait on the
  handle, you don't know it finished). What's different is two words:
  `tokio::spawn` instead of `thread::spawn`, and `.await` instead of
  `.join()`."*
- Say why that swap matters, tying back to segment 4.1.0's rule of thumb:
  *"threads for CPU-bound work, async for I/O-bound work. `consume` here
  sleeps to simulate exactly the kind of work a real background consumer
  actually does -- an HTTP call, a database write, waiting on a message
  broker. That's I/O-bound, so it belongs on a task, not a thread. Spawn a
  thousand of these as OS threads and you've got a thousand stacks sitting
  around mostly idle, waiting. Spawn a thousand as Tokio tasks and they
  share a small pool of real threads, each one just... not running, until
  its `sleep` or `recv` actually has something to do."*
- Tease the applied segment: *"this is a lesson sample. The next segment
  is this exact pattern, doing a real job, for the whole time this app
  runs."*

### 8. Applied for Real: Timing Out a Slow Query (~9 min -- the anchor segment)
**Files:** `src/routes/tasks.rs`

1. Open `get_task` and `with_timeout` together: *"`with_timeout` is
   generic over any fallible future -- the exact `tokio::select!` shape from
   the lesson sample, pulled out once so any endpoint in this file could
   reuse it instead of copy-pasting it into the handler."* Point out the
   unit tests directly beneath it: *"these don't touch the database at all
   -- a fake future that sleeps for a controlled duration is enough to
   prove both branches of the race. And notice they're not even marked
   `#[ignore]` for being slow -- there's no real sleep to wait out here at
   all, same virtual-clock trick as segment 2."*
2. Read `with_timeout`'s doc comment out loud, don't paraphrase it --
   *"this is the segment 6 cliffhanger getting its actual answer. `db::
   get_task` is a read, and `sqlx`'s pool detects a connection that got
   dropped mid-query and discards it instead of handing it back for reuse
   -- so cancelling this specific future can't leave some *other* request
   reading a corrupted response off a connection this one abandoned
   half-way through."* Then be honest about how that claim was checked, not
   just asserted: *"and that wasn't just trusted because it sounds right --
   it was checked by hand: hammering this endpoint with a 1-nanosecond
   timeout, guaranteeing every single query gets cancelled mid-flight,
   never once produced a corrupted response or a `500` -- only clean
   `504`s, over and over."*
3. Point at the doc comment's other admission, the honesty check: *"and
   right above that -- in real code, you'd reach for `tokio::time::timeout`
   instead of hand-rolling this with `select!`. It does the same thing in
   one line. This function exists to put the lesson's own primitive to
   work at a real call site, not because hand-rolling it here is actually
   the better engineering choice."*
4. Run the app (`cargo run`), create a task if none exists, then:
   ```sh
   curl -i http://127.0.0.1:8080/tasks/1
   # HTTP/1.1 200 OK
   ```
   *"three seconds of headroom, invisible on a healthy database -- this is
   the boring, correct case."*
5. **Live demo -- edit `GET_TASK_TIMEOUT` to `Duration::from_millis(1)`,**
   `cargo run` again, then the same `curl`:
   ```sh
   curl -i http://127.0.0.1:8080/tasks/1
   # HTTP/1.1 504 Gateway Timeout
   ```
   Say while it's on screen: *"one millisecond loses that race against any
   real database round trip, every time -- which is exactly the point.
   `tokio::select!` doesn't know or care *why* the loser lost; it just
   proceeds with whichever future resolved first and drops the other. In
   production this protects the request, and everything upstream of it,
   from one slow query hanging indefinitely -- and we just spent two
   minutes establishing exactly why dropping that query is safe to do."*
   **Revert `GET_TASK_TIMEOUT` to `Duration::from_secs(3)` and confirm the
   normal `curl` succeeds again before moving on.**

### 9. Applied for Real: the Background Async Consumer (~6 min -- the second anchor segment)
**Files:** `src/consumer.rs`, `src/main.rs`, `src/routes/tasks.rs`

1. Open `consumer.rs` top to bottom and draw the parallel to `audit.rs`
   explicitly, by name: *"same shape as the audit trail from segment 4.1.8
   -- a plain, non-`async`, non-blocking `publish` a handler calls without
   waiting on it, and a background consumer that owns the receiving end.
   Every difference from here on is 'async instead of threaded,' not
   anything new."*
2. Point at `EventPublisher::publish` specifically: *"not `async fn`.
   `UnboundedSender::send` never blocks -- there's no `.await` here to
   write, same as `AuditLogger::record`. The moment this function returns,
   the request handler moves on; nothing about processing this event has
   happened yet."*
3. Open `main.rs`'s two background workers side by side and read the
   shutdown lines: *"`audit_worker.shutdown()` -- a method on a type this
   app wrote. `event_consumer.await` -- straight on the `JoinHandle`,
   nothing custom at all. Both do the same job: wait for a background
   consumer to actually finish before the process exits. `tokio::task::
   JoinHandle` just doesn't need a wrapper to make that safe to call once,
   the way the audit worker's asymmetry -- many loggers, one worker --
   made worth wrapping there."*
4. Open the "Swapping in a real message broker" section of `consumer.rs`'s
   doc comment and read the `rdkafka` snippet aloud: *"this block never
   compiles -- it's marked `ignore` deliberately, `rdkafka` isn't a
   dependency of this app. It's here to answer the question you should be
   asking right now: what would actually change if this were a real Kafka
   topic? One thing: how `run` gets its next message. `consumer.subscribe`
   instead of building a channel, `stream.next().await` instead of
   `receiver.recv().await`. Spawning it -- `tokio::spawn`, once, at
   startup -- doesn't change at all."*
5. Run the app (`cargo run`), create a task, and point at the log line
   that appears roughly 5ms later (the simulated I/O in `process`):
   ```sh
   curl -s -X POST http://127.0.0.1:8080/tasks \
     -H 'content-type: application/json' -d '{"title": "ship it"}'
   ```
   ```text
   {"level":"INFO","fields":{"message":"processed task event","task_id":"1","kind":"task.created"}, ...}
   ```
   *"the HTTP response came back before this line was even written --
   `curl` already had its `201 Created` while this was still `sleep`ing.
   That's the entire point of a background consumer: the request doesn't
   wait around for whatever happens next."*

---

## Wrap-up (~3 min)

- Recap in one sentence each: threads and channels for message-passing
  concurrency, `Arc<Mutex<T>>` for shared state, `Send`/`Sync` for the
  compiler-checked reason any of it is safe, `thread::scope` for borrowing
  without `Arc`, Tokio for the same shapes under I/O-bound async work,
  `tokio::select!` for racing two things against each other, and
  `tokio::spawn` for a background async worker that runs the whole time
  your app does -- capped by the same "keep it small, real, and applied
  once" pattern Module 3's `ffi.rs` set: a background thread in `audit.rs`,
  its async sibling in `consumer.rs`, one race in `with_timeout`, nothing
  sprinkled into the handlers that don't need it.
- Point at the **Exercises** section of this module's `README.md` --
  explicitly say you're not solving them on camera.
- Tease Module 5 with the same continuation framing used to open every
  module so far: *"four modules in, and every single one has added either a
  new guarantee the compiler enforces for you, or a new tool for working
  within those guarantees. Next module turns back toward the web layer
  itself: middleware, and moving this app's persistence forward -- the
  parts of a real API that have nothing to do with the language and
  everything to do with the shape of the system around it."*

---

## Timing summary

| Segment | Target |
|---|---|
| Cold open | 2 min |
| 4.0.1 Closures: Capturing & the Three Traits | 8 min |
| 4.0.2 `move`, and Why Every Thread Needs It | 7 min |
| **Lesson 4.0 subtotal** | **15 min** |
| 4.1.0 Threads vs. Async, and What Channels Don't Solve | 2 min |
| 4.1.1 Spawning Threads | 3 min |
| 4.1.2 Thread Joining | 3 min |
| 4.1.3 Channel Basics | 3 min |
| 4.1.4 Channel Iteration | 3 min |
| 4.1.5 Shared State with `Arc<Mutex<T>>` | 5 min |
| 4.1.6 `Send` and `Sync` (live demo) | 6 min |
| 4.1.7 Scoped Threads | 4 min |
| 4.1.8 Applied for Real (Audit Trail) | 7 min |
| **Lesson 4.1 subtotal** | **36 min** |
| 4.2.1 Tokio Runtime | 2 min |
| 4.2.2 Spawning Async Tasks | 4 min |
| 4.2.3 Async Channels | 4 min |
| 4.2.4 Task Scheduling | 4 min |
| 4.2.5 Shared Async State | 6 min |
| 4.2.6 Racing Tasks with `tokio::select!` (cancellation safety) | 5 min |
| 4.2.7 Background Async Worker | 4 min |
| 4.2.8 Applied for Real (Query Timeout, live demo) | 9 min |
| 4.2.9 Applied for Real (Background Async Consumer) | 6 min |
| **Lesson 4.2 subtotal** | **44 min** |
| Wrap-up | 3 min |
| **Total** | **~100 min** |

If you're cutting to fit a tighter cap, trim 4.0 first -- keep 4.0.2 (`move`,
the bit the rest of the module depends on) and compress 4.0.1 to just the
three-way `Fn`/`FnMut`/`FnOnce` distinction. Then cut 4.2.1 to a one-line
callout, then cut 4.1.7 (scoped threads) to a mention without running the
sample. Don't cut 4.1.6's `compile_fail` demo or any of the three
applied-for-real segments (4.1.8, 4.2.8, 4.2.9) -- they're the highest
information-per-minute moments in the recording, the same role Module 3's
`ETag` `curl` walkthrough played. If you need to drop one applied segment
entirely rather than trim, drop 4.2.9 and fold its two-sentence gist (this
is the same audit-trail shape, just async, and here's what a real broker
would change) into 4.2.7's wrap-up instead -- it's the most self-contained
of the three.
