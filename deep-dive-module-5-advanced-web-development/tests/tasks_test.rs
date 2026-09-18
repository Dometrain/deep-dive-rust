use actix_web::{http::StatusCode, test, web, App};
use serde_json::{json, Value};
use todo_web_app::{
    cache::RecentTasksCache,
    models::{Task, TaskId},
    routes::tasks,
};

mod common;

const RECENT_TASKS_CAPACITY: usize = 10;

#[actix_web::test]
async fn create_list_and_get_task() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({
            "title": "Learn Rust",
            "description": "Finish the final project",
            "due_date": "2030-01-01T12:00:00Z"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let created: Task = test::read_body_json(response).await;
    assert_eq!(created.id, TaskId(1));
    assert_eq!(created.title.as_str(), "Learn Rust");
    assert!(!created.completed);
    assert_eq!(
        created
            .due_date
            .expect("due date should be returned")
            .to_rfc3339(),
        "2030-01-01T12:00:00+00:00"
    );

    let request = test::TestRequest::get().uri("/tasks").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let tasks: Vec<Task> = test::read_body_json(response).await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, created.id);

    let request = test::TestRequest::get().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let fetched: Task = test::read_body_json(response).await;
    assert_eq!(fetched.title, created.title);
}

#[actix_web::test]
async fn partial_update_preserves_omitted_fields() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({
            "title": "Original title",
            "description": "Keep this description",
            "due_date": null
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let request = test::TestRequest::put()
        .uri("/tasks/1")
        .set_json(json!({ "completed": true }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let updated: Task = test::read_body_json(response).await;
    assert_eq!(updated.title.as_str(), "Original title");
    assert_eq!(
        updated.description.map(|d| d.to_string()),
        Some("Keep this description".to_string())
    );
    assert!(updated.completed);
    assert!(updated.due_date.is_none());
}

#[actix_web::test]
async fn delete_task_and_report_missing_task() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({ "title": "Delete me" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let request = test::TestRequest::delete().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let request = test::TestRequest::get().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"], "Task not found");
}

#[actix_web::test]
async fn reject_blank_titles_on_create_and_update() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({ "title": "   " }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({ "title": "x".repeat(201) }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({ "title": "Valid title" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let request = test::TestRequest::put()
        .uri("/tasks/1")
        .set_json(json!({ "title": "" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn reject_due_dates_in_the_past() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({
            "title": "Time travel",
            "due_date": "2000-01-01T00:00:00Z"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn malformed_dates_and_oversized_payloads_are_rejected() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({
            "title": "Invalid date",
            "due_date": "not-an-iso-8601-date"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let oversized_body = json!({
        "title": "Oversized payload",
        "description": "x".repeat(17_000)
    })
    .to_string();
    let request = test::TestRequest::post()
        .uri("/tasks")
        .insert_header(("content-type", "application/json"))
        .set_payload(oversized_body)
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[actix_web::test]
async fn recent_tasks_lists_created_ids_most_recent_first() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    for title in ["First", "Second", "Third"] {
        let request = test::TestRequest::post()
            .uri("/tasks")
            .set_json(json!({ "title": title }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let request = test::TestRequest::get().uri("/tasks/recent").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let recent: Vec<TaskId> = test::read_body_json(response).await;
    assert_eq!(recent, vec![TaskId(3), TaskId(2), TaskId(1)]);
}

#[actix_web::test]
async fn get_task_returns_a_stable_etag_that_changes_when_the_task_changes() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/tasks")
        .set_json(json!({ "title": "Track my ETag" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let request = test::TestRequest::get().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    let first_etag = response
        .headers()
        .get("ETag")
        .expect("GET /tasks/{id} should set an ETag header")
        .to_str()
        .unwrap()
        .to_string();

    let request = test::TestRequest::get().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    let second_etag = response.headers().get("ETag").unwrap().to_str().unwrap();
    assert_eq!(
        first_etag, second_etag,
        "an unchanged task should keep the same ETag"
    );

    let request = test::TestRequest::put()
        .uri("/tasks/1")
        .set_json(json!({ "completed": true }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let request = test::TestRequest::get().uri("/tasks/1").to_request();
    let response = test::call_service(&app, request).await;
    let third_etag = response.headers().get("ETag").unwrap().to_str().unwrap();
    assert_ne!(
        first_etag, third_etag,
        "completing the task should change its ETag"
    );
}

#[actix_web::test]
async fn update_and_delete_missing_tasks_return_not_found() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    let request = test::TestRequest::put()
        .uri("/tasks/999")
        .set_json(json!({ "completed": true }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let request = test::TestRequest::delete().uri("/tasks/999").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Module 4's applied-for-real feature: `POST /tasks` sends an event over a
/// channel to `audit::AuditLogger`'s background thread instead of writing
/// it inline, so this test has to tolerate that thread not having caught up
/// the instant `POST /tasks` returns -- see the retry loop below, and
/// `audit::tests` for the same background-thread-plus-channel shape tested
/// without that timing wrinkle (by joining the worker directly instead of
/// going through HTTP).
#[actix_web::test]
async fn audit_log_records_every_created_task() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
            .app_data(web::Data::new(common::test_audit_logger()))
            .configure(tasks::configure),
    )
    .await;

    for title in ["First", "Second"] {
        let request = test::TestRequest::post()
            .uri("/tasks")
            .set_json(json!({ "title": title }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // The audit worker processes events off the request path, so give it a
    // few short retries to catch up rather than asserting on the very first
    // read -- a fixed `sleep` would either be flaky (too short) or slow
    // this test down for no reason (too long).
    let mut events: Vec<Value> = Vec::new();
    for _ in 0..20 {
        let request = test::TestRequest::get().uri("/tasks/audit").to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        events = test::read_body_json(response).await;
        if events.len() >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["task_id"], 1);
    assert!(events[0]["message"].as_str().unwrap().contains("First"));
    assert_eq!(events[1]["task_id"], 2);
    assert!(events[1]["message"].as_str().unwrap().contains("Second"));
}
