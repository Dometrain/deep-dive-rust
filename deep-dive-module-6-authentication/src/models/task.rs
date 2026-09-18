use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;

/// Non-empty, length-bounded title text. Callers can only ever hold a
/// `TaskTitle` that already satisfies these invariants -- there is no way to
/// construct one that doesn't, so handlers and queries never need to
/// re-validate a title once they have one. See `models::traits` for the
/// `Displayable`/`Storable`/`Validatable` traits built on top of this type.
pub const MAX_TITLE_CHARS: usize = 200;

/// Length-bounded description text (may be empty, unlike [`TaskTitle`]).
pub const MAX_DESCRIPTION_CHARS: usize = 5_000;

/// Newtype wrapper around a validated task id.
///
/// A bare `i64` would let a caller accidentally pass a task's id where a
/// page size, a limit, or any other `i64` was expected -- the newtype
/// pattern closes that hole at the type level for zero runtime cost (the
/// wrapper compiles down to the same `i64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TaskId(pub i64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for TaskId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// Newtype wrapper that makes "empty or overlong title" unrepresentable.
///
/// Validation happens once, in [`TaskTitle::try_from`], and every other
/// piece of code -- the HTTP handlers, the `sqlx` queries, the tracing spans
/// -- works with a `TaskTitle` that is guaranteed already valid. Deserializing
/// straight into this type (`#[serde(try_from = "String")]`) means an
/// invalid title never even makes it out of the JSON extractor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(try_from = "String")]
#[sqlx(transparent)]
pub struct TaskTitle(String);

impl TaskTitle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TaskTitle {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput("Title cannot be empty".into()));
        }
        if trimmed.chars().count() > MAX_TITLE_CHARS {
            return Err(AppError::InvalidInput(format!(
                "Title cannot exceed {MAX_TITLE_CHARS} characters"
            )));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl fmt::Display for TaskTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype wrapper that makes "overlong description" unrepresentable.
/// Unlike [`TaskTitle`], an empty description is allowed -- the invariant
/// this type enforces is only the length bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(try_from = "String")]
#[sqlx(transparent)]
pub struct TaskDescription(String);

impl TryFrom<String> for TaskDescription {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(AppError::InvalidInput(format!(
                "Description cannot exceed {MAX_DESCRIPTION_CHARS} characters"
            )));
        }
        Ok(Self(value))
    }
}

impl fmt::Display for TaskDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, PartialEq)]
pub struct Task {
    pub id: TaskId,
    pub title: TaskTitle,
    pub description: Option<TaskDescription>,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
}

/// A borrowed view of a task's title -- Module 2's "lifetime annotations in
/// structs" sample (`examples::module_2::lifetime_annotations_in_structs`),
/// applied to the real `Task` instead of a standalone `Excerpt`.
///
/// [`Task::summary`] builds one of these instead of an owned `String`, so
/// logging a task (see `cache::RecentTasksCache` usage in
/// `routes::tasks::create_task`) doesn't allocate. `'a` ties `TaskSummary`
/// to the `Task` it borrowed from -- the compiler won't let a `TaskSummary`
/// outlive the task whose title it points at.
#[derive(Debug, Clone, Copy)]
pub struct TaskSummary<'a> {
    pub title: &'a str,
}

impl fmt::Display for TaskSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

impl Task {
    pub fn summary(&self) -> TaskSummary<'_> {
        TaskSummary {
            title: self.title.as_str(),
        }
    }

    /// A weak "did this task change?" fingerprint, sent as the `ETag` header
    /// on `GET /tasks/{id}` (see `routes::tasks::get_task`) -- Module 3's
    /// FFI lesson, applied for real. `crate::ffi::checksum_hex` is the only
    /// function in this codebase's production code path that contains an
    /// `unsafe` block; everything here is a safe caller of it.
    ///
    /// Built from `Display`, not `Debug` or JSON, so the fingerprint only
    /// changes when a field's *rendered* value changes -- not when, say, a
    /// struct gains a field with a value that happens to serialize
    /// identically either way.
    pub fn etag(&self) -> String {
        let fingerprint = format!(
            "{}|{}|{}|{}|{:?}",
            self.id,
            self.title,
            self.description
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            self.completed,
            self.due_date,
        );
        crate::ffi::checksum_hex(fingerprint.as_bytes())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: TaskTitle,
    pub description: Option<TaskDescription>,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<TaskTitle>,
    pub description: Option<TaskDescription>,
    pub completed: Option<bool>,
    pub due_date: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed() {
        let title = TaskTitle::try_from("  Learn Rust  ".to_string()).unwrap();
        assert_eq!(title.as_str(), "Learn Rust");
    }

    #[test]
    fn blank_and_oversized_titles_are_rejected() {
        assert!(TaskTitle::try_from("   ".to_string()).is_err());
        assert!(TaskTitle::try_from("x".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }

    #[test]
    fn oversized_descriptions_are_rejected() {
        assert!(TaskDescription::try_from("x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn empty_description_is_allowed() {
        assert!(TaskDescription::try_from(String::new()).is_ok());
    }

    #[test]
    fn summary_borrows_the_title_without_allocating() {
        let task = Task {
            id: TaskId(1),
            title: TaskTitle::try_from("Learn Rust".to_string()).unwrap(),
            description: None,
            completed: false,
            created_at: Utc::now(),
            due_date: None,
        };

        let summary = task.summary();
        // Same address as `task.title`'s backing storage -- `summary` really
        // is a borrow, not a copy.
        assert_eq!(summary.title.as_ptr(), task.title.as_str().as_ptr());
        assert_eq!(summary.to_string(), "Learn Rust");
    }

    fn task(completed: bool) -> Task {
        Task {
            id: TaskId(1),
            title: TaskTitle::try_from("Learn Rust".to_string()).unwrap(),
            description: None,
            completed,
            created_at: Utc::now(),
            due_date: None,
        }
    }

    #[test]
    fn etag_is_stable_for_an_unchanged_task() {
        let task = task(false);
        assert_eq!(task.etag(), task.etag());
    }

    #[test]
    fn etag_changes_when_a_field_changes() {
        assert_ne!(task(false).etag(), task(true).etag());
    }
}
