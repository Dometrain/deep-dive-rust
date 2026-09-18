//! Shared behaviour for task-shaped types, expressed as traits.
//!
//! This is the "real" application of Module 1's associated types, trait
//! bounds and supertraits (as opposed to the standalone teaching snippets in
//! [`crate::examples`]) -- these traits are implemented for the actual
//! `Task`, `CreateTaskRequest` and `UpdateTaskRequest` types used by the
//! HTTP layer and the database layer.

use crate::error::AppError;
use chrono::Utc;
use std::fmt;

/// Anything that can render itself as a human-readable summary, independent
/// of `Debug`/`Display`. Kept as its own trait (rather than requiring
/// `Display`) so a type can offer a `Debug`-style dump *and* a friendlier
/// `display()` summary for logs without the two competing for the same
/// `fmt::Formatter` implementation.
pub trait Displayable {
    fn display(&self) -> String;
}

/// Anything that is identified by an id.
///
/// `Id` is an **associated type**, not a generic parameter: a given
/// `Storable` implementation has exactly one id type, so callers write
/// `T::Id` / `impl Storable<Id = TaskId>` rather than threading an extra
/// generic parameter through every function that touches storable things.
pub trait Storable {
    type Id;

    fn id(&self) -> Self::Id;
}

/// Anything that can validate whole-object invariants that a single field's
/// newtype can't express on its own -- for example, a due date that is
/// structurally a valid timestamp (`chrono` already guarantees that) but
/// falls in the past.
///
/// `Validatable: fmt::Display + fmt::Debug` is a **supertrait** bound: it
/// says "you can only implement `Validatable` for a type that also
/// implements `Display` and `Debug`". That lets `validate_and_log` below
/// require just `T: Validatable` and still call `%item` / `?item` on it.
pub trait Validatable: fmt::Display + fmt::Debug {
    fn validate(&self) -> Result<(), AppError>;
}

/// Elided-lifetime companion to `Displayable`: returns a *borrowed* summary
/// instead of an owned `String`.
///
/// `examples::module_2::lifetime_bounds_in_traits::Summary<'a>` spells its
/// lifetime out explicitly, because its implementor (`NewsArticle<'a>`)
/// holds borrowed fields directly, independent of any particular `&self`
/// call. `Task` owns its data instead, so the returned reference only ever
/// needs to live as long as the `&self` borrow used to get it -- exactly
/// lifetime elision rule 3, so there's no `<'a>` to write at all.
pub trait Summarize {
    fn summarize(&self) -> &str;
}

impl Summarize for Task {
    fn summarize(&self) -> &str {
        self.title.as_str()
    }
}

/// A **supertrait** composing the read-side behaviour a persisted task
/// exposes. Requiring `T: TaskEntity` is shorthand for
/// `T: Displayable + Storable`, so call sites that need both no longer
/// juggle two separate bounds.
pub trait TaskEntity: Displayable + Storable {}

impl<T: Displayable + Storable> TaskEntity for T {}

/// Logs a one-line summary of any persisted entity that implements
/// [`TaskEntity`]. `T::Id: fmt::Display` combines a **trait bound** on an
/// **associated type** -- generic over any entity, as long as its id can be
/// printed.
pub fn describe<T>(item: &T) -> String
where
    T: TaskEntity,
    T::Id: fmt::Display,
{
    format!("[id={}] {}", item.id(), item.display())
}

use super::task::{CreateTaskRequest, Task, UpdateTaskRequest};

impl Displayable for Task {
    fn display(&self) -> String {
        let status = if self.completed { "done" } else { "open" };
        format!("Task {}: {} ({status})", self.id, self.summarize())
    }
}

impl Storable for Task {
    type Id = super::task::TaskId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl fmt::Display for CreateTaskRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CreateTaskRequest(title = {})", self.title)
    }
}

impl Validatable for CreateTaskRequest {
    fn validate(&self) -> Result<(), AppError> {
        validate_due_date(self.due_date)
    }
}

impl fmt::Display for UpdateTaskRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UpdateTaskRequest(id unknown, title = {:?})", self.title)
    }
}

impl Validatable for UpdateTaskRequest {
    fn validate(&self) -> Result<(), AppError> {
        validate_due_date(self.due_date)
    }
}

/// Cross-field business rule that no single newtype could express: a due
/// date is a perfectly valid `DateTime<Utc>` even when it's in the past, so
/// this has to live at the whole-request level rather than on a field type.
fn validate_due_date(due_date: Option<chrono::DateTime<Utc>>) -> Result<(), AppError> {
    if let Some(due_date) = due_date {
        if due_date < Utc::now() {
            return Err(AppError::InvalidInput(
                "Due date cannot be in the past".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::TaskTitle;
    use chrono::Duration;

    fn request(due_date: Option<chrono::DateTime<Utc>>) -> CreateTaskRequest {
        CreateTaskRequest {
            title: TaskTitle::try_from("Learn Rust".to_string()).unwrap(),
            description: None,
            due_date,
        }
    }

    #[test]
    fn future_due_date_is_valid() {
        assert!(request(Some(Utc::now() + Duration::days(1)))
            .validate()
            .is_ok());
    }

    #[test]
    fn past_due_date_is_rejected() {
        assert!(request(Some(Utc::now() - Duration::days(1)))
            .validate()
            .is_err());
    }

    #[test]
    fn missing_due_date_is_valid() {
        assert!(request(None).validate().is_ok());
    }

    #[test]
    fn summarize_and_display_agree_on_the_title() {
        use crate::models::task::{Task, TaskId};
        use chrono::Utc;

        let task = Task {
            id: TaskId(1),
            title: TaskTitle::try_from("Learn Rust".to_string()).unwrap(),
            description: None,
            completed: false,
            created_at: Utc::now(),
            due_date: None,
        };

        assert_eq!(task.summarize(), "Learn Rust");
        assert!(task.display().contains(task.summarize()));
    }
}
