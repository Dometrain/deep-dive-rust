use actix_web::{http::StatusCode, test, web, App};
use serde_json::{json, Value};
use todo_traits_app::{
    models::{Task, TaskId},
    routes::tasks,
};

mod common;

#[actix_web::test]
async fn create_list_and_get_task() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
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
async fn update_and_delete_missing_tasks_return_not_found() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
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
