// Re-exports for the models module
pub mod task;
pub mod traits;

pub use task::{
    CreateTaskRequest, Task, TaskDescription, TaskId, TaskSummary, TaskTitle, UpdateTaskRequest,
};
pub use traits::{describe, Displayable, Storable, Summarize, TaskEntity, Validatable};
