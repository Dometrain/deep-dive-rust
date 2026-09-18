use crate::{
    cache::RecentTasksCache,
    db::{self, DbPool},
    error::AppError,
    models::{describe, CreateTaskRequest, TaskId, UpdateTaskRequest, Validatable},
};
use actix_web::{error::JsonPayloadError, http::StatusCode, web, HttpResponse, ResponseError};
use serde::Serialize;
use tracing::{debug, instrument};

const MAX_JSON_BYTES: usize = 16 * 1024;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(json_config()).service(
        web::scope("/tasks")
            .route("", web::get().to(list_tasks))
            .route("", web::post().to(create_task))
            // Registered ahead of "/{id}" on principle -- Actix's router
            // prefers a literal segment over a dynamic one regardless of
            // registration order, but a task can never actually be titled
            // "recent" since ids are numeric, so there's no real ambiguity
            // here either way.
            .route("/recent", web::get().to(recent_tasks))
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
    let task = db::get_task(pool.get_ref(), TaskId::from(id.into_inner())).await?;
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
