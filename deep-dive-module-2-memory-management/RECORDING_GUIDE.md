# Recording Guide -- Module 2: Advanced Memory Management

Target runtime: **60 minutes** (2.1: 30 min, 2.2: 30 min). Timings are
per-segment targets, not hard limits -- if a segment runs long, cut from
"extra context" first, never from the live code.

Two things should be open before you start recording:

- This folder (`deep-dive-module-2-memory-management`) in the editor, with
  `src/examples/module_2.rs`, `src/cache.rs`, `src/models/task.rs`,
  `src/models/traits.rs`, `src/routes/tasks.rs` and `src/main.rs` in tabs.
- A terminal in this folder, plus `deep-dive-module-1-advanced-traits` open
  in a second tab/window -- this module is a direct continuation, not a new
  app, and you'll want to show that.

Pre-recording checklist:

- [ ] `docker compose up -d db` run at least once (image pull is slow the
      first time).
- [ ] `cargo test --lib` passes (fast, no DB).
- [ ] Have `curl` (or similar) ready for the live demos in 2.1.5.
- [ ] Know how to trigger a compile error on demand -- see 2.1.5's "swap it
      back" demo, which needs you to comment/uncomment two lines live.

---

## Cold open (~2 min)

Don't open on `Box`. Open on something the audience has already been
running, unexplained, since **Module 6**: `grep -n Arc src/db/connection.rs`
on screen shows `pub type DbPool = Arc<PgPool>;` -- say plainly, *"this
line has been in every module since 6. Nobody's told you what `Arc` means
yet. By the end of today it will look completely different to you."* That's
the whole thesis of the module: smart pointers aren't new machinery you're
about to bolt on, they're a name for something already load-bearing in code
you've already shipped.

Then frame the shape of the 60 minutes: *"2.1 is about **sharing** and
**mutating** data safely without the compiler's usual ownership rules
getting in the way. 2.2 is about **proving to the compiler** that a
reference is safe without a garbage collector's help. Different problems,
same underlying question: how does Rust know this is safe, given it
doesn't have a GC watching at runtime?"*

---

## Lesson 2.1 -- Smart Pointers (`Box`, `Rc`, `RefCell`) (30 min)

### 0. What's a Smart Pointer? (~2 min)

Say it before any code: *"a smart pointer is a struct that acts like a
reference -- you can dereference it to get at the data inside -- but also
owns some extra behavior beyond just pointing at memory. A plain `&T` does
exactly one thing: borrow. `Box<T>`, `Rc<T>` and `RefCell<T>` each add one
specific extra capability on top of that -- heap placement, shared
ownership, runtime-checked mutability -- and today's four samples are one
smart pointer each, plus one that's two of them stacked together."* Don't
define more than that yet -- each segment below earns its own capability.

### 1. `Box<T>` for Heap Allocation (~5 min)
**File:** `src/examples/module_2.rs`, `mod box_heap_allocation`

- Ground it against something every viewer already knows if they've
  written C#: *"in .NET, every `class` instance already lives on the heap
  -- automatically, you never opt in. `struct` is the one that's special
  (stack/inline). Rust flips that default: everything lives on the stack
  (or inline inside whatever owns it) unless you explicitly ask for the
  heap. `Box::new(x)` is that explicit ask -- 'put this one value on the
  heap, and give me back a pointer to it that owns it, frees it when
  dropped, and derefs like a reference.'"*
- Now the concrete reason you're forced to reach for it here: try to write
  `ListNode { value: i32, next: Option<ListNode> }` with no `Box` on
  screen, and ask out loud *"how big is a `ListNode`?"* -- it contains a
  `ListNode`, which contains a `ListNode`, forever. The compiler can't
  compute a finite size, so this is a compile error, not a slow program.
  `Box<Option<ListNode>>` fixes it because a `Box` is always pointer-sized
  (8 bytes) no matter what it points to -- the recursion happens on the
  heap, one heap allocation at a time, instead of inline in the struct's
  own layout.
- Run `cargo test --lib examples::module_2::box_heap_allocation` and walk
  `collect_values` -- point out it's just following `.next` pointers, the
  same shape as any linked list in any language, the heap allocation is
  just explicit here instead of automatic.

### 2. `Rc<T>` for Shared Ownership (~5 min)
**File:** `src/examples/module_2.rs`, `mod rc_shared_ownership`

- The naive alternative first, so `Rc` has something to solve: *"normal
  Rust ownership says one value has exactly one owner. If two parts of
  your program both need the same `Task`, your only tool so far is
  `.clone()` -- which works, but now you have two independent copies, and
  a change to one doesn't show up in the other. `Rc<T>` -- 'reference
  counted' -- gives you multiple owners of the *same* value instead."*
- The .NET comparison, stated honestly: *"this is what .NET's garbage
  collector is already doing for you, for every object, automatically,
  the whole time you've been writing C#. `Rc<T>` is what it looks like to
  opt into that same idea manually and narrowly -- for one value, counted
  by hand, with no GC thread running in the background -- instead of
  getting it globally for free."*
- Run `cargo test --lib examples::module_2::rc_shared_ownership` and point
  at `Rc::strong_count` -- say plainly: *"this number is the whole
  mechanism. `Rc::clone` increments it, `drop` decrements it, and the data
  is freed the instant it hits zero -- deterministically, not whenever a
  GC thread gets around to it."*
- Flag it early so segment 5 lands harder later: *"single-threaded only --
  that count is a plain integer, not updated atomically. Two threads
  incrementing it at once is a data race. Hold that thought."*

### 3. `RefCell<T>` for Interior Mutability (~6 min)
**File:** `src/examples/module_2.rs`, `mod refcell_interior_mutability`

- This is the segment with no clean .NET analogue -- say so directly,
  don't force a comparison that doesn't fit: *"C# never stops you from
  mutating through a reference. There's no 'shared references are
  read-only' rule to work around, so there's nothing in C# that plays the
  role `RefCell` plays here. This tool exists because Rust has a rule C#
  doesn't."*
- State the rule being worked around, plainly: *"Rust's normal borrowing
  rule is one mutable reference, XOR any number of shared references --
  never both at once -- checked at compile time. `MockDatabase::execute_query`
  takes `&self`, not `&mut self`, but still needs to push onto a `Vec`
  inside. `RefCell` is how you tell the compiler 'stop checking this one
  at compile time -- I'll prove it's safe at runtime instead.'"*
- Show `borrow()` / `borrow_mut()` and say what they really are: *"the
  exact same one-writer-XOR-many-readers rule, just moved from compile
  time to runtime. Break it at compile time, you get a compiler error.
  Break it at runtime, through `RefCell`, you get a panic."* Then run
  `a_second_overlapping_mutable_borrow_panics` on screen and let viewers
  watch it actually panic -- don't just assert this happens, show it.

### 4. Combining `Rc` and `RefCell` (~4 min)
**File:** `src/examples/module_2.rs`, `mod combining_rc_refcell`

- Frame this as composition, not a new concept: *"`Rc<T>` alone gives you
  shared ownership of something immutable. `RefCell<T>` alone gives you
  runtime-checked mutability, but only one owner. Nest them --
  `Rc<RefCell<T>>` -- and you get both at once: many owners, any of whom
  can mutate the shared data."*
- Run the test and narrate `store.borrow_mut()` vs `store_clone.borrow_mut()`
  -- two different `Rc` handles, same underlying `RefCell`, same
  underlying `Vec`. Point at `Rc::strong_count(&store) == 2` as proof
  they're the same allocation, not two copies.

### 5. Use in To-Do App -- and the `Arc<Mutex<_>>` pivot (~8 min -- the anchor segment)
**Files:** `src/examples/module_2.rs::refactor_todo_app`, `src/cache.rs`,
`src/routes/tasks.rs`, `src/main.rs`

This is the segment to protect on time. Walk it as a real "here's why the
lesson's own sample would be a mistake to ship" story, in this order:

1. Show `examples::module_2::refactor_todo_app::single_threaded` --
   this is the lesson doc's sample, verbatim: `Vec<Rc<RefCell<Task>>>`.
   Run its test, let it pass. Say plainly: *"this compiles, this test
   passes, and putting it in `src/main.rs` as shared app state would still
   be unsound. Watch."*
2. Open `src/routes/tasks.rs` and `src/main.rs` and show the real feature
   this module actually shipped: `RecentTasksCache`, registered as
   `web::Data` in `main.rs`, recorded into from `create_task`, read from
   the new `GET /tasks/recent` route. Run the app (`cargo run`), create a
   couple of tasks with `curl`, then `curl http://127.0.0.1:8080/tasks/recent`
   and show the ids come back most-recent-first.
3. **Live compile-error demo -- don't skip this, it's the strongest single
   moment in the recording. Verified against the real compiler before
   writing this, so the error text below is exact, not a guess.** In
   `src/cache.rs`, temporarily swap `Arc` -> `Rc` and `Mutex` -> `RefCell`
   throughout (`use std::sync::{Arc, Mutex}` -> `use std::rc::Rc; use
   std::cell::RefCell;`, `Arc<Inner>` -> `Rc<Inner>`, `Mutex<VecDeque<...>>`
   -> `RefCell<VecDeque<...>>`, `.lock().unwrap()` -> `.borrow_mut()` /
   `.borrow()`). Run `cargo check --all-targets`. Two errors appear, both
   worth reading out loud:
   - `cache.rs`'s own `stays_correct_when_recorded_from_another_thread`
     test fails first, with `` `Rc<Inner>` cannot be sent between threads
     safely `` pointing straight at the `std::thread::spawn` call --
     because that test was written in segment 5 specifically to prove
     thread-safety, it's the first thing to notice the type stopped being
     thread-safe.
   - `src/main.rs`'s `HttpServer::new(move || { ... })` fails the same way:
     `` `Rc<cache::Inner>` cannot be sent between threads safely ``,
     because `HttpServer` requires its factory closure to be `F: Fn() -> I
     + Send + Clone + 'static` -- and the closure captures `recent_tasks`,
     which now contains an `Rc`.
   Say: *"neither of these is a runtime bug you'd find by testing --
   they're both caught before the program exists. This is 2.1.2's warning
   -- 'single-threaded only' -- made real."* Revert with
   `git checkout -- src/cache.rs` (or undo manually), then confirm
   `cargo check --all-targets` is clean again before moving on.
4. Close the loop on 2.1.2's `Arc::strong_count` idea: point at
   `#[derive(Clone)] pub struct RecentTasksCache(Arc<Inner>)` and the
   `recent_tasks.clone()` call in `main.rs`'s `HttpServer::new(move || ...)`
   closure -- *"this runs once per worker thread. Every worker gets its
   own `RecentTasksCache` value, but they all share the same `Arc<Inner>`
   underneath -- same relationship as `Rc::clone` in segment 2, just safe
   to hand across threads because the count is atomic and the data's
   behind a lock, not a runtime-checked flag."*

Close 2.1 with the bridge into 2.2: *"everything so far has been about
**what happens while the program runs** -- reference counts, runtime
borrow checks, locks. Lifetimes are the opposite: they're Rust proving
something *before* the program ever runs at all."*

---

## Lesson 2.2 -- Advanced Lifetimes (30 min)

### 0. Why Lifetimes Exist (No GC) (~3 min)

Don't open on syntax -- open on the C# contrast, because it's the cleanest
possible motivation: *"in C#, you never think about how long a reference
stays valid. The garbage collector won't collect an object while anything
still points to it -- safety is enforced by keeping data alive **longer**
if needed, decided at runtime. Rust has no GC. It has to prove the same
safety property -- no reference ever points at freed memory -- a different
way: the compiler checks, before your program ever runs, that every
reference's lifetime fits inside the data it points to. Lifetimes aren't
a runtime mechanism at all -- they're what the compiler uses to do that
proof, and once it succeeds, they disappear. Zero runtime cost, same as
the newtypes from Module 1."*

Then reconnect to something already on the audience's hard drive:
*"Module 3 already showed you `fn longest<'a>(x: &'a str, y: &'a str) -> &'a str`
-- lifetimes on a function. Everything in the next 27 minutes is that same
`'a` syntax, just showing up in three new places: on a struct, on a trait,
and as a rule for when you're allowed to leave it out entirely."*

### 1. Lifetime Annotations in Structs (~7 min)
**Files:** `src/examples/module_2.rs::lifetime_annotations_in_structs`,
then `src/models/task.rs`

- Show `struct Excerpt<'a> { part: &'a str }` and read the `<'a>` in plain
  terms: *"this struct holds a reference, and `'a` is a name for 'however
  long that reference happens to be valid.' The compiler won't let you
  build an `Excerpt` whose `part` is a dangling pointer, and it won't let
  an `Excerpt` outlive the string it borrowed from -- `'a` is the leash
  connecting the two."*
- Run the test, point at `&novel[..16]` -- the `Excerpt` borrows a slice
  of `novel`; if `novel` were dropped while `first_sentence` was still
  around, this wouldn't compile at all. That's the whole guarantee, made
  concrete.
- Now the real one: open `src/models/task.rs`, find `TaskSummary<'a>` and
  `Task::summary(&self) -> TaskSummary<'_>`. Say what changed and why:
  *"`describe()` from Module 1 returns an owned `String` -- it allocates
  every single time you call it. `TaskSummary` borrows the title instead
  -- zero allocation. `'_` here just means 'infer the lifetime from
  context' -- it's still `Excerpt<'a>`'s exact same shape, Rust just
  doesn't make you spell `'a` out when there's only one reasonable
  choice."* Run `models::task::tests::summary_borrows_the_title_without_allocating`
  and point at the `as_ptr()` assertion: *"this test is checking the
  address, not just the value -- proving `summary.title` really does point
  at the same bytes as `task.title`, not a copy of them."*

### 2. Lifetime Bounds in Traits (~7 min)
**Files:** `src/examples/module_2.rs::lifetime_bounds_in_traits`, then
`src/models/traits.rs`

- Show `trait Summary<'a> { fn summarize(&self) -> &'a str; }` and be
  precise about what's different from segment 1: *"`'a` here belongs to
  the *trait*, not tied to `&self`. `NewsArticle<'a>` holds `headline: &'a str`
  directly -- the string it returns from `summarize` was never borrowed
  from `&self` in the first place, it was already sitting in the struct,
  borrowed from wherever `NewsArticle` was originally built. The lifetime
  has to be spelled out because it doesn't come from the method call at
  all."*
- Now open `src/models/traits.rs` and show `Summarize` --
  `fn summarize(&self) -> &str` with **no** lifetime written anywhere. Ask
  the question out loud before answering it: *"why does this one get to
  skip `<'a>` entirely?"* Answer: *"`Task` owns its title -- a `TaskTitle`,
  which owns a `String`. The only borrow happening is `&self` itself, so
  the returned reference just needs to live as long as that one borrow.
  That's a single, unambiguous choice, which is exactly what lifetime
  elision rule 3 covers next segment -- so the compiler fills it in for
  you and there's nothing to write."* This is the moment that ties 2.2.1,
  2.2.2 and 2.2.3 into one story: same underlying idea, three different
  amounts of syntax depending on where the data actually lives.

### 3. Lifetime Elision Rules (~6 min)
**File:** `src/examples/module_2.rs::lifetime_elision`

- State the three rules once, plainly, before the code: *"one, every
  reference parameter gets its own lifetime. Two, if there's exactly one
  input lifetime, every output reference gets that same one. Three, if one
  of the inputs is `&self` or `&mut self`, every output gets `self`'s
  lifetime instead. These are pattern-matched against your function
  signature in order -- if one of them applies cleanly, you can leave the
  `'a` out and the compiler fills it in silently."*
- Show `fn first_word(s: &str) -> &str` and apply rule 2 out loud, one
  step at a time: *"one input reference (`s`) -> rule 2 says the output
  gets the same lifetime -> fully written out, this is
  `fn first_word<'a>(s: &'a str) -> &'a str`. Nobody writes it that way
  because the compiler does it for you."*
- Callback, don't introduce new code: *"`TaskTitle::as_str(&self) -> &str`
  in `models/task.rs` -- one of the very first things this course
  showed you, back in Module 1 -- is rule 3, not rule 2: the input is
  `&self`, so the output borrows from `self`. You've been reading elided
  lifetimes since before this module existed; today's the first time
  you've had the rule that explains why it compiles."*

### 4. Variance (~7 min -- closing segment)
**File:** `src/examples/module_2.rs::variance`

Say up front this one is different in kind from the other three: *"you
will not consciously write variance-related code in this app, today or
later. It matters because it explains borrow-checker errors you'll
eventually hit involving generics and lifetimes together, not because
you'll reach for it directly. Treat this as reading comprehension for
future compiler errors, not a new tool."*

- **Covariance -- the one with a real, runnable proof.** Show `shortest<'a>`
  and the test: a `&'static str` and a shorter-lived `&str` both get
  passed in, and it compiles. Say plainly: *"a `&'static str` -- valid for
  the whole program -- can stand in anywhere a `&'a str` for some shorter
  `'a` is expected. A longer-lived reference is always usable as a
  shorter-lived one. That's covariance: `&'static str` is a *subtype* of
  `&'a str` for every `'a`, the same relationship `string` and `object` in
  C# have, just for lifetimes instead of class hierarchies."*
- **Contravariance -- be honest that this is prose, not a demo.** Say it
  directly, don't fake a proof: *"the lesson doc's own sample for this --
  a plain function taking `Fn(&i32) -> i32` -- doesn't actually exercise
  contravariance at all, there's no lifetime subtyping in it anywhere.
  A genuine example needs two function *pointer types* compared against
  each other, and the direction only shows up in what the type checker
  accepts, not in a value you can assert on in a test."* Read the comment
  block in the file: a function pointer that accepts *any* lifetime is
  usable wherever one that only accepts `'static` was expected -- the
  reverse direction from the reference case above. That reversal is the
  whole definition of "contra."
- **Invariance -- the one with the clearest "why."** Point at the comment
  showing the `Cell<&'static str>` -> `Cell<&'a str>` coercion that Rust
  refuses to allow. Walk the consequence out loud: *"if that were allowed,
  you could `.set()` a short-lived reference into a `Cell` whose original
  owner still believes it only ever holds `'static` strings -- a dangling
  reference the moment the short-lived one's `'a` ends. `Cell`, `RefCell`
  and `Mutex` -- this lesson's own interior-mutability types from 2.1 --
  are invariant over their contents specifically to rule that out. This is
  variance and interior mutability meeting each other: the reason `Cell<T>`
  can't be covariant is *because* it lets you write through a shared
  reference."* That last sentence is worth landing slowly -- it's the one
  place all of 2.1 and 2.2 touch directly.
- Wrap by re-running the full suite on screen (`cargo test --lib`, 35
  passing) as the "everything you just saw is real, tested code" closer --
  same closing beat Module 1 used.

---

## Wrap-up (~3 min)

- Recap in one sentence each: `Box` for heap placement and recursive
  types, `Rc`/`Arc` for shared ownership (single- vs. multi-threaded),
  `RefCell`/`Mutex` for interior mutability (runtime-checked vs. locked),
  lifetimes for compiler-proven reference safety with no GC, elision for
  when that proof is unambiguous enough to skip writing, variance for why
  some of those proofs succeed or fail once generics are involved.
- Point at the **Exercises** section of this module's `README.md` --
  explicitly say you're not solving them on camera.
- Tease Module 3 (Unsafe Rust) with the same framing used to open both
  modules so far: *"we've been asking the compiler to prove memory safety
  for us this whole course. Next module is what happens when you tell it
  to stop."*

---

## Timing summary

| Segment | Target |
|---|---|
| Cold open | 2 min |
| 2.1.0 What's a Smart Pointer? | 2 min |
| 2.1.1 `Box<T>` | 5 min |
| 2.1.2 `Rc<T>` | 5 min |
| 2.1.3 `RefCell<T>` | 6 min |
| 2.1.4 Combining `Rc`/`RefCell` | 4 min |
| 2.1.5 Use in To-Do App (`Arc<Mutex<_>>` pivot) | 8 min |
| **Lesson 2.1 subtotal** | **30 min** |
| 2.2.0 Why Lifetimes Exist | 3 min |
| 2.2.1 Lifetime Annotations in Structs | 7 min |
| 2.2.2 Lifetime Bounds in Traits | 7 min |
| 2.2.3 Lifetime Elision Rules | 6 min |
| 2.2.4 Variance | 7 min |
| **Lesson 2.2 subtotal** | **30 min** |
| Wrap-up | 3 min |
| **Total** | **~65 min** (60 min lesson content + 5 min cold open/wrap-up) |

If you're cutting to fit a hard 60-minute cap, cut from 2.1.5's step 4
(the `Arc::strong_count` callback -- steps 1-3 carry the segment) and
2.2.4's contravariance prose (read it once, don't re-derive it live) first.
Don't cut the live compile-error demo in 2.1.5 step 3 -- it's the highest
information-per-minute moment in the recording.
