# Deep Dive: Rust -- Module 3: Unsafe Rust & Macros

The same to-do API as
[`deep-dive-module-2-memory-management`](../deep-dive-module-2-memory-management)
-- this module **starts from that codebase and keeps building on it**, the
same way Module 2 started from
[`deep-dive-module-1-advanced-traits`](../deep-dive-module-1-advanced-traits).
Every type, trait and smart pointer Module 1 and Module 2 added (`TaskId`,
`TaskTitle`, `Displayable`, `Storable`, `RecentTasksCache`, `TaskSummary<'a>`,
...) is still here, unchanged. This module adds raw pointers, `unsafe`
blocks and functions, and a real C FFI call -- plus **macros**
(`macro_rules!`, and what `#[derive(...)]` generates), the other "advanced
language feature" the book pairs with `unsafe`. The topics of *Deep Dive:
Rust*, Module 3.

## Prerequisites

- Rust (stable, edition 2021)
- A C compiler on `PATH` -- `cc`, `gcc` or `clang`. This module links a small
  C library at build time (see below), and the `cc` crate needs one to
  compile it.
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

The server listens on `http://127.0.0.1:8080`. Configuration and the
`/tasks` CRUD endpoints are unchanged from Module 1 and 2 -- see Module 1's
README for the full table and `curl` examples.

## What's new in Module 3

| Change | Added by |
|---|---|
| `ETag` response header on `GET /tasks/{id}` | This module -- a checksum of the task's fields, computed via the one FFI call this app makes in production |

```sh
curl -i http://127.0.0.1:8080/tasks/1
# HTTP/1.1 200 OK
# ETag: "3fa1c9de"
# ...
```

## Where each lesson sample lives

Run every standalone sample with:

```sh
cargo test --lib
```

| Lesson | Submodule | Sample |
|---|---|---|
| 3.1 | `examples::module_3::raw_pointer_basics` | `*const T` / `*mut T` created from the same value, read and written through inside `unsafe` |
| 3.1 | `examples::module_3::dereferencing_raw_pointers` | A safe write-then-read through a valid pointer, alongside a pointer that's allowed to dangle -- and is deliberately never dereferenced |
| 3.1 | `examples::module_3::unsafe_functions` | An `unsafe fn` with a documented `# Safety` precondition, wrapped in a safe `safe_divide` |
| 3.1 | `examples::module_3::ffi_with_c` | `extern "C"` calls into `add` and `print_message` from [`native/native.c`](native/native.c) |
| 3.1 | `examples::module_3::passing_data_to_c` | Passing a `&str` to C via `CString`, and receiving a `#[repr(C)]` struct back from C |
| 3.1 | `examples::module_3::linking_a_c_library` | No new code -- explains why every sample above already demonstrates this lesson |
| 3.2 | `examples::module_3::macros` | A declarative `macro_rules!` (`my_vec!`), and a `#[derive(...)]` struct proving derives are generated code -- not runtime reflection |

## What's different from Module 2: why raw pointers and `unsafe` never appear in `src/routes` or `src/main.rs`

Every request this API handles is: deserialize JSON (`serde`), validate a
few fields (`models::traits::Validatable`), run a query (`sqlx`), serialize
JSON back. None of that has a legitimate reason to reach for a raw pointer
or an `unsafe` block directly -- `serde`, `sqlx` and `actix-web` already
*contain* the `unsafe` code this kind of program needs, written and audited
by people who specialize in it, sitting behind APIs designed so callers like
this codebase never have to write `unsafe` themselves. Adding a hand-rolled
`unsafe` block to a route handler wouldn't be "using Module 3's lesson" --
it would be reintroducing, by hand and without a specialist's scrutiny,
exactly the class of bug those libraries exist to keep out of application
code.

So instead of forcing unsafe/FFI into the CRUD handlers where it doesn't
belong, [`src/ffi.rs`](src/ffi.rs) adds one small, real, narrowly-scoped
feature: `GET /tasks/{id}` now returns an `ETag` header, a checksum of the
task's fields computed by a tiny C function
([`native/native.c`](native/native.c)'s `checksum`) and linked in by
[`build.rs`](build.rs) via the [`cc`](https://docs.rs/cc) crate. This is a
defensible, if deliberately modest, real-world reason to reach for FFI: a
fast, non-cryptographic checksum is exactly the kind of tight numeric loop
C (or a hand-written `unsafe` Rust routine) is good at, and the entire
`unsafe` surface it introduces is one function,
[`Task::etag`](src/models/task.rs) calling
[`ffi::checksum_hex`](src/ffi.rs) calling one `extern "C"` function whose own
signature -- a pointer and a length in, a `u32` out -- has no way to
misbehave for any byte slice a caller hands it. Grep this codebase's
non-test, non-`examples` source for the word `unsafe` and `ffi.rs`'s one
block is the only match.

## Running the tests

Unit tests (everything in the table above, plus `cache`, `ffi`,
`models::task` and `models::traits`) don't need a database, but they do
need a C compiler on `PATH` the first time (see
[Prerequisites](#prerequisites)) -- `build.rs` compiles
[`native/native.c`](native/native.c) once per `cargo build`/`cargo test`
invocation whose inputs changed, and the compiled object is reused after
that:

```sh
cargo test --lib
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

## Exercises

Not pre-solved here -- try them against this codebase:

1. `examples::module_3::unsafe_functions::dangerous_divide` isn't really
   memory-unsafe (see the module doc's honesty note). Write a *genuinely*
   unsafe function -- one whose documented precondition, if violated, would
   read past the end of a `Vec` -- and a safe wrapper around it that checks
   the precondition before calling it.
2. Add a `Weak`-free, `unsafe`-free way to answer "how many times has
   `checksum_hex` been called?" using only what Module 2 gave you
   (`Arc<Mutex<u64>>`, incremented inside `ffi::checksum_hex`). Then explain
   in a comment why this doesn't need `unsafe` even though it's tracking
   global state a C program might use a raw global counter for.
3. `native/native.c`'s `create_string` returns a pointer into a `static`
   string literal, which is why `passing_data_to_c::receive_from_c` never
   has to free anything. Add a second C function that instead calls
   `malloc` for its buffer, add the matching `free` call on the Rust side,
   and explain in a comment exactly which side is now responsible for
   freeing the memory and why getting that wrong would leak or double-free.
4. Add a C function to `native/native.c` that multiplies two integers, an
   `extern "C"` declaration and safe wrapper for it next to `add_via_c`, and
   a test proving it works.
5. `Task::etag` folds every field except `created_at` into its fingerprint.
   Is that a bug or a deliberate choice? Write a test that pins down which
   one it is, then adjust the doc comment on `etag` to say so explicitly.
