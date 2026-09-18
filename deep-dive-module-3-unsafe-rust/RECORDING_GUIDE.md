# Recording Guide -- Module 3: Unsafe Rust

Target runtime: **50 minutes** of lesson content (two lessons: 3.1 Unsafe/FFI,
and 3.2 Macros -- the book groups these two "advanced features" together, and
so do we).
Timings are per-segment targets, not hard limits -- if a segment runs long,
cut from "extra context" first, never from the live code.

Two things should be open before you start recording:

- This folder (`deep-dive-module-3-unsafe-rust`) in the editor, with
  `src/examples/module_3.rs`, `native/native.c`, `build.rs`, `src/ffi.rs`,
  `src/models/task.rs` and `src/routes/tasks.rs` in tabs.
- A terminal in this folder, plus `deep-dive-module-2-memory-management`
  open in a second tab/window -- this module is a direct continuation, not
  a new app, same as Module 2 was to Module 1.

Pre-recording checklist:

- [ ] A C compiler is on `PATH` (`cc --version`) -- if this is a fresh
      machine, `cargo build` will fail with a `cc` crate error, not a
      helpful one, so check this before you're rolling.
- [ ] `docker compose up -d db` run at least once (image pull is slow the
      first time).
- [ ] `cargo test --lib` passes (fast, no DB) -- also warms the C build
      cache so you don't sit through a `cc` compile mid-recording.
- [ ] Have `curl` (or similar) ready for the `ETag` demo.
- [ ] Know how to trigger the `dangling_pointers_from_locals` warning on
      demand -- see segment 2's live demo, which needs you to comment out
      one `#[allow(...)]` line.

---

## Cold open (~2 min)

Open by finishing Module 2's own sentence. That module closed with: *"we've
been asking the compiler to prove memory safety for us this whole course.
Next module is what happens when you tell it to stop."* Say that line again,
then land the actual thesis: *"'stop' doesn't mean 'nothing is checked
anymore.' It means exactly one specific set of checks turns off inside an
`unsafe` block -- and the trade you're making is: you now personally
guarantee, by reading the code, what the compiler used to guarantee by
construction. Get it right and nothing looks different from the outside.
Get it wrong and you get undefined behavior -- not a panic, not an error
message, just a program that's allowed to do anything at all."*

Frame the shape of the 30 minutes: *"one lesson, four ideas that build on
each other in a straight line: raw pointers (what you're allowed to create),
dereferencing them (the one operation that's actually unsafe), `unsafe`
functions (how you package that up and hand a safe interface to everyone
else), and FFI (the single most common legitimate reason to need any of the
above -- calling code that isn't Rust, and was never checked by *any*
Rust rule at all)."*

---

## Lesson 3.1 -- Raw Pointers and FFI (30 min)

### 0. What Does `unsafe` Actually Turn Off? (~2 min)

Say the exact list before any code, so segment 1 lands as a checklist being
worked through rather than a vague warning: *"an `unsafe` block lets you do
four things the compiler otherwise refuses: dereference a raw pointer, call
an `unsafe fn` (including an `extern` one), mutate a `static`, and implement
an `unsafe trait`. That's it. Everything else -- the type system, ownership,
moves -- is still fully checked inside `unsafe {}`. It's a scalpel for four
specific rules, not an off switch for the language."*

### 1. Raw Pointer Basics (~4 min)
**File:** `src/examples/module_3.rs`, `mod raw_pointer_basics`

- Show `*const i32` and `*mut i32` created from `&x` / `&mut x` and say
  plainly: *"creating a raw pointer isn't unsafe at all -- you can do this
  outside any `unsafe` block. Watch what the borrow checker does and
  doesn't do here."* Point at `raw_ptr_const` and `raw_ptr_mut` existing at
  the same time and say: *"with real references, `&x` and `&mut x` alive
  together, with the shared one used again afterward, is exactly the
  aliasing violation the borrow checker exists to catch. With raw pointers,
  it compiles -- because the borrow checker's job stops the moment you cast
  to `*const T` / `*mut T`. The rule about not aliasing mutable and shared
  access to the same data hasn't gone away, it's just yours to enforce now,
  by reading the code instead of the compiler reading it for you."*
- Run `cargo test --lib examples::module_3::raw_pointer_basics` and walk
  the one `unsafe` block: *"reading through `raw_ptr_const`, writing through
  `raw_ptr_mut`, reading it back -- three uses of the four things `unsafe`
  unlocks, all in four lines."*

### 2. Dereferencing Raw Pointers (~6 min -- has a live demo)
**File:** `src/examples/module_3.rs`, `mod dereferencing_raw_pointers`

- Run `write_and_read_back` first -- the boring, safe case, on screen,
  passing. Say: *"this is the whole 'access the data behind a pointer'
  idea. Nothing interesting happens when the pointer is valid. Interesting
  is what happens when it isn't."*
- Walk `dangling_pointer` line by line: *"`x` is a local. `&x as *const i32`
  is fine on its own line. Then the function returns, `x`'s stack slot is
  gone, and the pointer we handed back still holds that now-meaningless
  address."* Ask out loud: *"would this compile with `&i32` instead of
  `*const i32`?"* -- no, "returns a value referencing data owned by the
  current function," a lifetime error, at compile time. *"With a raw
  pointer, there's no lifetime attached at all, so there's nothing for the
  compiler to check -- and it compiles."*
- **Live demo -- comment out `#[allow(dangling_pointers_from_locals)]`
  above `dangling_pointer` and run `cargo build`.** A real warning appears,
  pointing at exactly this line: `a dangling pointer will be produced
  because the local variable 'x' will be dropped`. Read it out loud, then
  say the honest, precise thing about it: *"recent rustc versions added a
  lint for this **specific shape** of mistake -- a raw pointer to a local,
  returned past the local's scope. It's a real, useful warning, and it is
  not the borrow checker. It's a heuristic that happens to catch this one
  pattern; it can't see every way to produce a dangling pointer (a pointer
  stored in a struct field, freed through an entirely different code path,
  is invisible to it). That gap -- between 'the compiler happens to catch
  this' and 'the compiler proves this' -- is `unsafe`'s whole reason to
  exist."* Restore the `#[allow(...)]` line and confirm `cargo build` is
  warning-free again before moving on.
- Close with the rule stated plainly: *"dereferencing this pointer -- not
  creating it, not returning it, the actual `*ptr` -- is undefined behavior.
  This codebase never does it, on purpose. The test only proves the pointer
  was created; showing you the crash would mean shipping code that
  reliably corrupts memory, and that's not a demo, that's a bug."*

### 3. `unsafe` Functions (~4 min)
**File:** `src/examples/module_3.rs`, `mod unsafe_functions`

- Show `dangerous_divide` and `safe_divide`, then immediately do the
  honesty check this module's own doc comment does: *"integer
  division-by-zero in Rust already panics safely, with no `unsafe` involved
  -- this isn't a memory-safety hazard. This sample is here to show you the
  **contract**, not a genuine danger: `unsafe fn` is Rust's way of saying
  'I have a precondition I can't express in the type system, and I'm
  trusting my caller to check it before calling me.'"*
- Point at the `# Safety` doc comment and say: *"this isn't decoration --
  `clippy` will warn if a public `unsafe fn` doesn't have one. It's the
  actual contract: 'here is what you, the caller, must guarantee.'"*
- Run both tests (`Some(5)`, `None` for division by zero) and land the
  pattern that matters most going forward: *"`safe_divide` checks the
  precondition once, then calls the `unsafe fn`. This is the shape every
  real `unsafe` usage in this codebase follows -- one narrow `unsafe fn` or
  block, wrapped immediately in a safe function that upholds its contract,
  so every other caller in the program never has to think about `unsafe`
  at all. You'll see it again in ten minutes, for real, in `ffi.rs`."*

### 4. FFI with C (~6 min)
**Files:** `native/native.c`, `build.rs`, `src/examples/module_3.rs::ffi_with_c`

- Open `native/native.c` first, not the Rust side: *"this is a completely
  ordinary, tiny C file. `add`, `print_message` -- nothing Rust-aware about
  it at all. FFI's whole premise is that this file doesn't know Rust
  exists."*
- Open `build.rs`: *"three lines. The `cc` crate -- a build-time-only
  dependency, see `Cargo.toml`'s `[build-dependencies]` -- compiles this
  `.c` file and statically links it into whatever binary or test gets built
  next. This runs automatically, every `cargo build` and `cargo test` --
  nobody types `gcc` by hand."*
- Show the `extern "C" { fn add(...); fn print_message(...); }` block and
  say what it really is: *"a promise, not a definition. Rust is taking your
  word for it that a function named `add`, with this exact signature,
  exists somewhere it'll be linked against. Get the signature wrong --
  wrong argument types, wrong return type -- and nothing catches it at
  compile time. That mismatch is undefined behavior the first time it's
  called, which is exactly why every call site is wrapped in `unsafe`."*
- Run `cargo test --lib examples::module_3::ffi_with_c` and point at the
  `SAFETY` comment on `add_via_c`: *"this one's `unsafe` block is almost
  vacuous -- `add` takes two `i32`s and returns one, there's no pointer
  anywhere in its signature for anyone to get wrong. `print_via_c` is the
  one worth reading closely."*
- Read the `CString` comment out loud -- it's a real, common footgun worth
  landing: *"a Rust byte string, `b"hi"`, is **not** null-terminated. C's
  `%s` reads until it finds a `\0` -- hand it a non-null-terminated buffer
  and it keeps reading into whatever memory happens to follow, until it
  gets lucky and finds a zero byte somewhere. `CString::new` is what
  actually appends that `\0` for you, and it also rejects a message that
  already contains one in the middle, which C would otherwise silently
  truncate at."*

### 5. Passing Data to C (~5 min)
**File:** `src/examples/module_3.rs::passing_data_to_c`

- Show `send_to_c` first (the easy direction) then `receive_from_c`. On the
  `#[repr(C)] struct RustString` declaration, call out the bug this course's
  own lesson doc had and how this file avoids it: *"a struct definition,
  with or without `#[repr(C)]`, is not an external item -- it can't live
  inside an `extern "C" { }` block, only the *function signature* that
  returns one can. `#[repr(C)]` on its own is the part doing real work: it
  tells rustc 'lay this struct out in memory exactly the way a C compiler
  would' -- field order, padding, alignment, all of it -- so the bytes
  `create_string` hands back mean the same thing on both sides of the FFI
  boundary."*
- Walk the `SAFETY` comment on `std::slice::from_raw_parts` slowly -- it's
  the most consequential unsafe call in the whole module: *"this function
  takes your word for two things: the pointer is valid for `len` reads
  right now, and that memory won't be mutated out from under the slice for
  as long as it exists. Neither is checked. Get `len` wrong -- too large --
  and you've just told safe Rust code it's allowed to read past the end of
  a buffer."*
- Run the tests, then land the API design point: *"once
  `std::str::from_utf8` succeeds, everything downstream of `receive_from_c`
  is ordinary, checked, safe Rust again. That's the goal every time: keep
  the unsafe surface as small and as early as possible, then hand safe code
  a safe value."*

### 6. Linking a C Library (~2 min)
**File:** `src/examples/module_3.rs::linking_a_c_library`

- This is the shortest segment on purpose -- say so: *"there's no new code
  here. `build.rs` already linked `native/native.c` in for every sample
  you've just watched run. This lesson is 'notice that already
  happened.'"* Run
  `cargo test --lib examples::module_3::linking_a_c_library` and point out
  the comment: if the link step had failed, nothing above this point in the
  recording would have compiled at all.

### 7. Applied for Real: an `ETag` from a C Checksum (~6 min -- the anchor segment)
**Files:** `src/ffi.rs`, `src/models/task.rs`, `src/routes/tasks.rs`

This is the segment to protect on time -- same role Module 2's
`Arc<Mutex<_>>` pivot played.

1. Open `README.md`'s "why raw pointers and `unsafe` never appear in
   `src/routes` or `src/main.rs`" section and read the first paragraph
   aloud: *"`serde`, `sqlx` and `actix-web` already contain the `unsafe`
   code a program like this needs. Adding more of it to a route handler
   wouldn't be practicing this lesson, it'd be reintroducing, by hand, the
   exact class of bug those libraries exist to keep out."*
2. Open `native/native.c`'s `checksum` function -- a small, ordinary loop,
   no pointer arithmetic beyond `data[i]`. Then `src/ffi.rs`'s
   `checksum_hex`: one `extern "C"` declaration, one `unsafe` block, one
   `SAFETY` comment. Say: *"grep this codebase's real (non-`examples`,
   non-test) source for the word `unsafe` and this is the only line that
   comes back."*
3. Open `src/models/task.rs`'s `Task::etag` and `src/routes/tasks.rs`'s
   `get_task` -- point out `etag` is computed and stored in a local
   *before* `task` moves into `.json(task)`, same "compute what you need,
   then let the value move" shape as `create_task`'s `task.summary()` call
   in Module 2.
4. Run the app (`cargo run`), create a task, then:
   ```sh
   curl -i http://127.0.0.1:8080/tasks/1
   ```
   Point at the `ETag` header in the response. Run the same `curl` again --
   same `ETag`. Then `PUT` a change (`completed: true`) and `curl` again --
   different `ETag`. Say: *"this is the entire point of an `ETag`: a
   cheap fingerprint a client can compare without re-fetching or
   re-parsing the whole body. `crate::ffi::checksum_hex` is why it's cheap
   -- a tight byte loop is exactly what C, or a hand-written `unsafe`
   routine, is good at, and it's also exactly the kind of narrow,
   self-contained job that's safe to hand off to code Rust can't see
   inside of."*

---

## Lesson 3.2 -- Macros (20 min)
**File:** `src/examples/module_3.rs`, `mod macros`

Frame the pivot: *"unsafe was about stepping *outside* the compiler's safety
checks. Macros are the mirror image -- stepping *into* its code generation.
Every `println!`, every `#[derive(...)]` you've written since Module 1, is a
macro: code written for you, at compile time. Three kinds."*

### 1. Declarative macros -- `macro_rules!` (~7 min)
**Submodule:** `macros::my_vec`

- Open `mod macros` and walk `my_vec!`. Point at the matcher `$( $item:expr
  ),*`: *"'zero or more expressions, comma-separated.' The body repeats once
  per match -- `push($item)` for each."*
- Say what it's for, and what it isn't: *"this is the machinery behind `vec!`,
  `println!`, `assert_eq!`. Reach for one only when a function *can't* do the
  job -- raw syntax, a variadic list, generating items. Functions have better
  errors; prefer them when they suffice. It's called with `!`."*
- Run `cargo test --lib examples::module_3::macros` -- green.

### 2. What `#[derive(...)]` generates (~7 min -- the anchor for this lesson)
**Submodule:** `macros::Config`

- This is the segment that pays off three modules of `#[derive(...)]`. Point at
  `#[derive(Default, Debug, Clone, PartialEq)]` on `Config`, then at the
  hand-written `impl Default` in the doc comment: *"a derive is a *procedural*
  macro -- it reads the struct's tokens and emits an `impl`, field by field.
  This is code you could have written by hand and never see."*
- Run the `derived_impls_are_generated_code_you_never_wrote` test and read it:
  `default()`, `clone()`, `==`, `{:?}` all work, none written by hand.
- **The .NET headline, said slowly:** *"`System.Text.Json` serialises by
  *reflecting* over a type at runtime. `#[derive(Serialize)]` generates the code
  at *compile* time -- so there's no reflection, it's faster, and a missing impl
  is a compile error, not a runtime exception. Rust's derives are the
  compile-time cousins of .NET **source generators**, not of
  attributes-plus-reflection."*
- Land the practical payoff: *"this is why `the trait Serialize is not
  implemented for MyField` isn't noise -- it means a *field's* type lacks the
  impl. Derive or implement it there."*

### 3. Attribute macros (~4 min)
**Reference:** the doc-comment note in `mod macros`; `#[tokio::main]` (next module), `#[tracing::instrument]` (Module 5).

- The third kind, briefly: *"an attribute macro rewrites the item it's attached
  to. `#[tokio::main]` turns your `async fn main` into a plain `fn main` that
  starts a runtime -- something a function call could never do. You'll meet it
  in the very next module."*
- Close the trio: *"three kinds, one line each -- declarative (`macro_rules!`,
  tokens to tokens), derive (generates an impl from a type), attribute (rewrites
  the item it's on). You use all three constantly; now you know which is which."*

---

## Wrap-up (~3 min)

- Recap in one sentence each: raw pointers for "not checked, not yet
  unsafe," dereferencing for the one operation that actually is, `unsafe
  fn` for packaging a precondition into a documented contract, FFI for
  calling code with no Rust rules attached to it at all, and the applied
  lesson -- keep the `unsafe` surface as small, as early, and as wrapped in
  a safe function as `safe_divide` and `checksum_hex` both are. Then macros as
  the flip side: three kinds, all compile-time, and `#[derive]` is generated
  code (a source generator), not runtime reflection.
- Point at the **Exercises** section of this module's `README.md` --
  explicitly say you're not solving them on camera.
- Tease Module 4 with the same continuation framing used to open every
  module so far: *"three modules in, and the one thing every single lesson
  has shared is that Rust makes you handle a failure mode explicitly,
  somewhere, instead of letting it happen silently. Next module's failure
  mode is the one every other mainstream language hands you at runtime, in
  production, under load: two threads touching the same data at the same
  time. Next module asks: what does the compiler actually do to make that
  category of bug impossible to ship, instead of just easy to avoid?"*

---

## Timing summary

| Segment | Target |
|---|---|
| Cold open | 2 min |
| 3.1.0 What Does `unsafe` Turn Off? | 2 min |
| 3.1.1 Raw Pointer Basics | 4 min |
| 3.1.2 Dereferencing Raw Pointers (live demo) | 6 min |
| 3.1.3 `unsafe` Functions | 4 min |
| 3.1.4 FFI with C | 6 min |
| 3.1.5 Passing Data to C | 5 min |
| 3.1.6 Linking a C Library | 2 min |
| 3.1.7 Applied for Real (`ETag`) | 6 min |
| **Lesson 3.1 subtotal** | **35 min** |
| 3.2.1 Declarative Macros (`macro_rules!`) | 7 min |
| 3.2.2 What `#[derive(...)]` Generates (anchor) | 7 min |
| 3.2.3 Attribute Macros | 4 min |
| **Lesson 3.2 subtotal** | **18 min** |
| Wrap-up | 3 min |
| **Total** | **~60 min** |

If you're cutting to fit a tighter cap, cut 3.1.6 to a one-line callout
instead of a separate segment, and cut 3.1.2's live-demo restoration step
(just mention you reverted it) first. Don't cut 3.1.2's warning demo itself
or 3.1.7's `curl` walkthrough -- they're the highest information-per-minute
moments in the recording.
