use crate::{
    audit::AuditLogger,
    cache::RecentTasksCache,
    consumer::EventPublisher,
    db::{self, DbPool},
    error::AppError,
    models::{describe, CreateTaskRequest, TaskId, UpdateTaskRequest, Validatable},
};
use actix_web::{error::JsonPayloadError, http::StatusCode, web, HttpResponse, ResponseError};
use serde::Serialize;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, instrument};

const MAX_JSON_BYTES: usize = 16 * 1024;

/// How long `get_task` will race the database query against before giving
/// up -- see `with_timeout` and Module 4's `tokio::select!` lesson
/// (`examples::module_4::racing_tasks_with_select`), applied for real.
const GET_TASK_TIMEOUT: Duration = Duration::from_secs(3);

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(json_config()).service(
        web::scope("/tasks")
            .route("", web::get().to(list_tasks))
            .route("", web::post().to(create_task))
            // Registered ahead of "/{id}" on principle -- Actix's router
            // prefers a literal segment over a dynamic one regardless of
            // registration order, but a task can never actually be titled
            // "recent" or "audit" since ids are numeric, so there's no real
            // ambiguity here either way.
            .route("/recent", web::get().to(recent_tasks))
            .route("/audit", web::get().to(audit_log))
            .route("/{id}", web::get().to(get_task))
            .route("/{id}", web::put().to(update_task))
            .route("/{id}", web::delete().to(delete_task)),
    );
}

/// Field-level invariants (non-empty title, length bounds, ...) are enforced
/// by the `TaskTitle`/`TaskDescription` newtypes at deserialize time
/// (`#[serde(try_from = "String")]`, see `models::task`), so a malformed
/// request never reaches a handler -- it fails inside the JSON extractor
/// instead. This error handler translates that failure into the same
/// `{"error": "..."}` shape `AppError` produces, so API consumers see one
/// consistent error format regardless of where the rejection happened.
fn json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .limit(MAX_JSON_BYTES)
        .error_handler(|err, _req| {
            let response = match &err {
                // A request with a `Content-Length` header over the limit
                // (`OverflowKnownLength`) and one without (`Overflow`) are
                // reported as two separate variants by this version of
                // actix-web -- matching only one left the other silently
                // falling into the generic `400` branch below.
                JsonPayloadError::Overflow { .. }
                | JsonPayloadError::OverflowKnownLength { .. } => {
                    crate::error::json_error(StatusCode::PAYLOAD_TOO_LARGE, "Payload too large")
                }
                _ => AppError::InvalidInput(err.to_string()).error_response(),
            };
            actix_web::error::InternalError::from_response(err, response).into()
        })
}

// Explicit span names: handler names collide with the db layer functions,
// and these spans nest inside the per-request span from tracing-actix-web.
#[instrument(name = "handler::list_tasks", skip(pool))]
async fn list_tasks(pool: web::Data<DbPool>) -> Result<HttpResponse, AppError> {
    let tasks = db::list_tasks(pool.get_ref()).await?;
    debug!(count = tasks.len(), "listed tasks");
    Ok(HttpResponse::Ok().json(tasks))
}

#[instrument(name = "handler::create_task", skip_all, fields(task.title = %request.title))]
async fn create_task(
    pool: web::Data<DbPool>,
    recent_tasks: web::Data<RecentTasksCache>,
    audit: web::Data<AuditLogger>,
    events: web::Data<EventPublisher>,
    request: web::Json<CreateTaskRequest>,
) -> Result<HttpResponse, AppError> {
    let request = request.into_inner();
    // Only the cross-field rule (due date not in the past) is left to check
    // here -- per-field rules were already enforced while deserializing.
    request.validate()?;

    let task = db::create_task(pool.get_ref(), request).await?;

    // `record` only ever locks its `Mutex` for a few instructions and never
    // across an `.await` -- see `cache::RecentTasksCache` for why that
    // matters (and why it isn't the `Rc<RefCell<_>>` the lesson reaches for
    // first).
    recent_tasks.record(task.id);
    debug!(summary = %task.summary(), "recorded in recent-tasks cache");

    // Module 4's threads + channels lesson, applied for real: this is a
    // channel send, not a database write or a lock held across an `.await`
    // -- see `crate::audit` for the background thread on the other end.
    audit.record(task.id, format!("created \"{}\"", task.title));

    // The async sibling of the line above: publishes onto the (simulated)
    // topic `consumer::run` is consuming in a background `tokio::spawn`ed
    // task instead of a background thread -- see `crate::consumer`.
    events.publish(task.id, "task.created");

    Ok(HttpResponse::Created().json(task))
}

/// The ids of the most recently created tasks, most recent first. Backed by
/// `cache::RecentTasksCache`, an in-memory, in-process cache -- restarting
/// the app clears it, and (unlike everything else in this API) it is never
/// persisted to Postgres.
#[instrument(name = "handler::recent_tasks", skip(recent_tasks))]
async fn recent_tasks(recent_tasks: web::Data<RecentTasksCache>) -> HttpResponse {
    HttpResponse::Ok().json(recent_tasks.snapshot())
}

#[instrument(name = "handler::get_task", skip(pool), fields(task.id = *id))]
async fn get_task(pool: web::Data<DbPool>, id: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let task_id = TaskId::from(id.into_inner());

    // Module 4's `tokio::select!` lesson
    // (`examples::module_4::racing_tasks_with_select`), applied for real:
    // race the database query against a fixed timeout so one slow query
    // can't hang this request indefinitely. See `with_timeout` below for
    // why cancelling `db::get_task` mid-flight, specifically, is safe.
    let task = with_timeout(GET_TASK_TIMEOUT, db::get_task(pool.get_ref(), task_id)).await?;
    debug!(task = %describe(&task), "fetched task");
    // `etag()` is computed before the response is built (and before `task`
    // moves into `.json`) -- see `models::task::Task::etag`, which is the
    // one call in this handler that ends up inside `crate::ffi`'s `unsafe`
    // block.
    let etag = task.etag();
    Ok(HttpResponse::Ok()
        .insert_header(("ETag", format!("\"{etag}\"")))
        .json(task))
}

/// Races `future` against a `duration`-long sleep and returns
/// [`AppError::Timeout`] if the sleep wins. Generic over `future`'s output
/// so any fallible async call in this module can be wrapped the same way
/// `get_task` wraps `db::get_task` above.
///
/// In real production code, reach for [`tokio::time::timeout`] instead of
/// hand-rolling this with `select!` -- it does the same thing (races your
/// future against a delay, cancelling whichever loses) in one line, and its
/// own implementation is built on the same primitives this function uses
/// directly. This module writes it out by hand specifically to apply the
/// `tokio::select!` lesson (`examples::module_4::racing_tasks_with_select`)
/// to a real call site, not because hand-rolling is the better choice here.
///
/// # Cancellation safety
///
/// Losing this race means `future` gets **dropped mid-flight**, not run to
/// completion in the background -- so wrapping a future in this (or in
/// `tokio::time::timeout`) is only sound if dropping it early can't corrupt
/// state or lose an effect that already happened. `db::get_task`
/// specifically is safe to cancel this way: it's a read, and `sqlx`'s
/// connection pool detects a connection dropped mid-query and discards it
/// rather than handing it back for reuse, so a cancelled query here can't
/// leave a later, unrelated request reading a stale or corrupted response
/// off the same connection (verified by hand: firing repeated rapid requests
/// against a 1-nanosecond timeout, guaranteeing every single query gets
/// cancelled mid-flight, never produced a wrong body or a `500` -- only
/// clean `504`s). That reasoning is specific to a read wrapped in
/// `sqlx`'s pool -- it doesn't generalize for free to every future; a
/// half-sent multi-part write, or anything else with a side effect that
/// isn't safely repeatable, needs its own case-by-case answer before it's
/// safe to race like this.
async fn with_timeout<F, T>(duration: Duration, future: F) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::select! {
        result = future => result,
        _ = tokio::time::sleep(duration) => Err(AppError::Timeout),
    }
}

/// The full audit trail of task-creation events recorded so far, most
/// recent last -- backed by `audit::AuditLogger`, an in-memory,
/// in-process log populated by a background thread. Like
/// `cache::RecentTasksCache`, restarting the app clears it.
#[instrument(name = "handler::audit_log", skip(audit))]
async fn audit_log(audit: web::Data<AuditLogger>) -> HttpResponse {
    HttpResponse::Ok().json(audit.snapshot())
}

#[instrument(name = "handler::update_task", skip(pool, request), fields(task.id = *id))]
async fn update_task(
    pool: web::Data<DbPool>,
    id: web::Path<i64>,
    request: web::Json<UpdateTaskRequest>,
) -> Result<HttpResponse, AppError> {
    let request = request.into_inner();
    request.validate()?;

    let task = db::update_task(pool.get_ref(), TaskId::from(id.into_inner()), request).await?;
    Ok(HttpResponse::Ok().json(task))
}

#[instrument(name = "handler::delete_task", skip(pool), fields(task.id = *id))]
async fn delete_task(
    pool: web::Data<DbPool>,
    id: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    db::delete_task(pool.get_ref(), TaskId::from(id.into_inner())).await?;
    Ok(HttpResponse::Ok().json(MessageBody {
        message: "Task deleted",
    }))
}

#[derive(Serialize)]
struct MessageBody<'a> {
    message: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fast_ok() -> Result<&'static str, AppError> {
        tokio::time::sleep(Duration::from_millis(5)).await;
        Ok("data")
    }

    async fn slow_ok() -> Result<&'static str, AppError> {
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok("data")
    }

    #[tokio::test]
    async fn returns_the_futures_result_when_it_finishes_before_the_timeout() {
        let result = with_timeout(Duration::from_millis(100), fast_ok()).await;
        assert_eq!(result.unwrap(), "data");
    }

    #[tokio::test]
    async fn returns_timeout_when_the_future_is_slower_than_the_deadline() {
        let result = with_timeout(Duration::from_millis(10), slow_ok()).await;
        assert!(matches!(result, Err(AppError::Timeout)));
    }
}
