use super::connection::DbPool;
use crate::{
    error::AppError,
    models::{describe, CreateTaskRequest, Task, TaskId, UpdateTaskRequest},
};
use tracing::{info, instrument};

#[instrument(
    skip(pool, task),
    err,
    fields(
        db.system.name = "postgresql",
        db.operation.name = "INSERT",
        db.collection.name = "tasks",
        db.query.summary = "INSERT INTO tasks",
        task.title = %task.title,
    )
)]
pub async fn create_task(pool: &DbPool, task: CreateTaskRequest) -> Result<Task, AppError> {
    let task = sqlx::query_as::<_, Task>(
        r#"
        INSERT INTO tasks (title, description, due_date)
        VALUES ($1, $2, $3)
        RETURNING id, title, description, completed, created_at, due_date
        "#,
    )
    .bind(task.title)
    .bind(task.description)
    .bind(task.due_date)
    .fetch_one(pool.as_ref())
    .await?;

    // `describe` only needs `T: TaskEntity` (i.e. `Displayable + Storable`)
    // plus a printable id -- it works for `Task` here but would work just as
    // well for any other entity that implements the same two traits.
    info!(task = %describe(&task), "created task");

    Ok(task)
}

#[instrument(
    skip(pool),
    err,
    fields(
        db.system.name = "postgresql",
        db.operation.name = "SELECT",
        db.collection.name = "tasks",
        db.query.summary = "SELECT FROM tasks",
        task.id = %id,
    )
)]
pub async fn get_task(pool: &DbPool, id: TaskId) -> Result<Task, AppError> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, title, description, completed, created_at, due_date
        FROM tasks
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool.as_ref())
    .await?
    .ok_or(AppError::TaskNotFound)
}

#[instrument(
    skip(pool),
    err,
    fields(
        db.system.name = "postgresql",
        db.operation.name = "SELECT",
        db.collection.name = "tasks",
        db.query.summary = "SELECT FROM tasks",
    )
)]
pub async fn list_tasks(pool: &DbPool) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>(
        r#"
        SELECT id, title, description, completed, created_at, due_date
        FROM tasks
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .fetch_all(pool.as_ref())
    .await?;

    Ok(tasks)
}

#[instrument(
    skip(pool, task),
    err,
    fields(
        db.system.name = "postgresql",
        db.operation.name = "UPDATE",
        db.collection.name = "tasks",
        db.query.summary = "UPDATE tasks",
        task.id = %id,
    )
)]
pub async fn update_task(
    pool: &DbPool,
    id: TaskId,
    task: UpdateTaskRequest,
) -> Result<Task, AppError> {
    let updated = sqlx::query_as::<_, Task>(
        r#"
        UPDATE tasks
        SET title = COALESCE($1, title),
            description = COALESCE($2, description),
            completed = COALESCE($3, completed),
            due_date = COALESCE($4, due_date)
        WHERE id = $5
        RETURNING id, title, description, completed, created_at, due_date
        "#,
    )
    .bind(task.title)
    .bind(task.description)
    .bind(task.completed)
    .bind(task.due_date)
    .bind(id)
    .fetch_optional(pool.as_ref())
    .await?
    .ok_or(AppError::TaskNotFound)?;

    Ok(updated)
}

#[instrument(
    skip(pool),
    err,
    fields(
        db.system.name = "postgresql",
        db.operation.name = "DELETE",
        db.collection.name = "tasks",
        db.query.summary = "DELETE FROM tasks",
        task.id = %id,
    )
)]
pub async fn delete_task(pool: &DbPool, id: TaskId) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(pool.as_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::TaskNotFound);
    }

    Ok(())
}
