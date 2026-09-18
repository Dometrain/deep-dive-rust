//! Consumer-driven contract tests between this backend and the `todo-shared`
//! wire types the Leptos frontend deserializes into.
//!
//! The backend and the frontend each have their *own* definition of a task --
//! the backend's `models::Task` is validated and `sqlx`-backed; the shared
//! `Task` is a flat serde struct. That's deliberate (see `mod-7.md`), but it
//! means the two can silently drift: rename a field on one side and the
//! frontend breaks only at runtime, in a browser, with a parse error no
//! backend test would ever see. These tests close that gap by round-tripping
//! each type through JSON into its counterpart and asserting they agree --
//! turning a runtime frontend break into a failed `cargo test` here.
//!
//! No database needed: this is pure serde, so it runs with `cargo test`
//! without a Postgres container.

use chrono::{TimeZone, Utc};
use todo_ws_app::models::{
    CreateTaskRequest, LoginResponse, Task, TaskDescription, TaskId, TaskTitle,
};

#[test]
fn backend_task_serializes_into_the_shared_task() {
    let backend_task = Task {
        id: TaskId(7),
        title: TaskTitle::try_from("Learn Leptos".to_string()).unwrap(),
        description: Some(TaskDescription::try_from("with a real frontend".to_string()).unwrap()),
        completed: true,
        created_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
        due_date: Some(Utc.with_ymd_and_hms(2030, 6, 7, 8, 9, 10).unwrap()),
    };

    // Exactly what a handler writes to the response body / a `/ws` broadcast.
    let json = serde_json::to_string(&backend_task).expect("backend Task should serialize");

    // Exactly what the frontend does with that body.
    let shared_task: todo_shared::Task =
        serde_json::from_str(&json).expect("shared Task should deserialize the backend's JSON");

    assert_eq!(shared_task.id, 7);
    assert_eq!(shared_task.title, "Learn Leptos");
    assert_eq!(
        shared_task.description.as_deref(),
        Some("with a real frontend")
    );
    assert!(shared_task.completed);
    assert_eq!(shared_task.created_at, backend_task.created_at);
    assert_eq!(shared_task.due_date, backend_task.due_date);
}

#[test]
fn a_shared_create_request_deserializes_into_the_backend_request() {
    // What the frontend sends when a user adds a task.
    let from_frontend = todo_shared::CreateTaskRequest {
        title: "  A brand new task  ".to_string(),
        description: None,
        due_date: None,
    };
    let json = serde_json::to_string(&from_frontend).expect("shared request should serialize");

    // The backend deserializes it into its validated newtype, which also
    // trims -- so a valid title from the frontend is accepted, and the
    // trimming the backend promises still happens.
    let backend_request: CreateTaskRequest =
        serde_json::from_str(&json).expect("backend should accept the frontend's request shape");
    assert_eq!(backend_request.title.as_str(), "A brand new task");
}

#[test]
fn login_response_shapes_match() {
    let backend = LoginResponse {
        token: "a.b.c".to_string(),
    };
    let json = serde_json::to_string(&backend).expect("backend LoginResponse should serialize");
    let shared: todo_shared::LoginResponse =
        serde_json::from_str(&json).expect("shared LoginResponse should deserialize it");
    assert_eq!(shared.token, "a.b.c");
}
