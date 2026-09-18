// Re-exports for the database module
pub mod connection;
pub mod queries;

pub use connection::{create_pool, initialize_database, DbPool};
pub use queries::{create_task, delete_task, get_task, list_tasks, update_task};
