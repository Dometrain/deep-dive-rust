# Deep Dive: Rust -- Module 1: Advanced Traits and Generics

The same Actix Web + SQLx + Postgres to-do API as
[`module-9-final-project`](../module-9-final-project), extended to demonstrate
associated types, default type parameters, trait bounds, the newtype pattern
and supertraits -- the topics of *Deep Dive: Rust*, Module 1.

## Prerequisites

- Rust (stable, edition 2021)
- A container runtime for Postgres and the integration tests:
  [OrbStack](https://orbstack.dev) or Docker

## Running locally

Start Postgres:

```sh
docker compose up -d db
```

Start the API (migrations run automatically on startup):

```sh
cargo run
```

The server listens on `http://127.0.0.1:8080` by default. Configuration and
the `/tasks` API are unchanged from Module 9 -- see that module's README for
the full endpoint table and `curl` examples.

## What's different from Module 9

Module 9 validated task fields with free functions (`validated_title`,
`validate_description`) called from inside the HTTP handlers, after the
JSON body had already been parsed into plain `String` fields. Here, the
same rules are pushed into the type system instead:

| Where | Module 9 | Module 1 |
|---|---|---|
| Task id | `i64` | `TaskId(i64)` -- a newtype, so an id can't be passed where an arbitrary `i64` is expected |
| Task title | `String`, checked by `validated_title()` after parsing | `TaskTitle`, a newtype whose `TryFrom<String>` *is* the only way to build one -- an invalid title can't exist |
| Task description | `Option<String>`, checked by `validate_description()` | `Option<TaskDescription>`, same idea |
| Shared behaviour | none -- each handler calls `db::*` functions directly | `Displayable`, `Storable` (associated `Id` type) and `Validatable` (a supertrait of `Display + Debug`) traits, composed via `TaskEntity` |

See [`src/models/task.rs`](src/models/task.rs) for the newtypes and
[`src/models/traits.rs`](src/models/traits.rs) for the traits, both fully
wired into [`src/routes/tasks.rs`](src/routes/tasks.rs) and
[`src/db/queries.rs`](src/db/queries.rs).

### Why a due date still needs runtime validation

Newtypes only catch invariants a *single field* can check by itself. "Title
isn't empty" fits that; "due date isn't in the past" doesn't -- a due date is
a perfectly valid `DateTime<Utc>` regardless of when "now" is. That rule
lives in `Validatable::validate()` on `CreateTaskRequest`/`UpdateTaskRequest`
instead, called once from the handler after the newtype-level checks have
already passed. This is the practical answer to the module's "when do I use
associated types/newtypes vs. a validation trait?" question: per-field,
always-true invariants go in a newtype's constructor; cross-field or
time-dependent rules go in a trait method.

### Error responses stay consistent

Because `TaskTitle`/`TaskDescription` now validate *during* JSON
deserialization (`#[serde(try_from = "String")]`), a bad title fails inside
Actix's JSON extractor rather than inside a handler. `routes::tasks::json_config`
installs a custom `error_handler` so that failure still comes back as the
same `{"error": "..."}` body `AppError` produces elsewhere in the API
(with the pre-existing 413 for oversized payloads still distinguished from
400 for a genuinely invalid body).

## Lesson code samples

[`src/examples.rs`](src/examples.rs) has one self-contained, unit-tested
submodule per code sample from the lesson -- each can be copied out on its
own:

| Lesson | Submodule | Sample |
|---|---|---|
| 1.1 | `examples::associated_types` | Trait with an associated type (`Collection`) |
| 1.1 | `examples::task_list` | Implementing an associated type for a `TaskList` |
| 1.1 | `examples::implementing_iterator` | Implementing `std`'s own associated-type trait (`Iterator`) -- write `next`, get every adaptor (`map`/`filter`/`collect`) for free |
| 1.1 | `examples::default_type_parameters` | Default type parameters (`Displayable<T = String>`) |
| 1.1 | `examples::trait_bounds` | Trait bounds (`T: Display`) |
| 1.1 | `examples::refactor_todo` | Shared behaviour via `Displayable` + `Storable` |
| 1.2 | `examples::newtype` | Newtype pattern (`Meters` / `Seconds`) |
| 1.2 | `examples::supertraits` | Supertraits (`Validatable: Display + Debug`) |
| 1.2 | `examples::validation_newtype` | Validation with a newtype (`NonEmptyString`) |
| 1.2 | `examples::composing_traits` | Composing traits (`TaskTraits`) |

Run them with:

```sh
cargo test --lib
```

## Running the tests

Unit tests (the table above, plus the newtype/validation tests in
`src/models/`) don't need a database:

```sh
cargo test --lib
```

The HTTP integration tests use [Testcontainers RS](https://testcontainers.com/)
to spin up a throwaway Postgres container per test, same as Module 9:

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

Run the full stack (app + Postgres + Jaeger) in containers:

```sh
docker compose up --build
```

The Jaeger UI is available at <http://localhost:16686>.

## Exercises

The lesson's five learner exercises aren't pre-solved here -- try them
against this codebase:

1. Extend `examples::implementing_iterator::TaskIdRange` with a `step` field so
   it can yield every *n*th id, then rewrite an explicit `for`/`push` loop
   elsewhere in this crate as an iterator chain and confirm the tests still
   pass. (Bonus: add an `Iter` associated type to
   `examples::associated_types::Collection` alongside `Item`, with an `iter()`
   method returning it.)
2. Give `examples::default_type_parameters::Displayable` a second, custom
   type parameter implementation (e.g. `impl Displayable<bool> for Task`
   returning whether the task is overdue).
3. Add a `PositiveInteger(u32)` newtype and use it for `Task::id` instead of
   `TaskId` -- what changes, and what doesn't?
4. Define a `Loggable: Display + Debug` supertrait and implement it for the
   real `Task` type in `src/models/`.
5. `src/models/traits.rs` already refactors the app this way -- read through
   it, then extend `Validatable` with a second cross-field rule of your own
   (e.g. reject a `completed: true` task with no `title`).
