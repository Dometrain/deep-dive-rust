use actix_web::{http::header, http::StatusCode, test, web, App};
use serde_json::{json, Value};
use todo_ws_app::{cache::RecentTasksCache, routes};

mod common;

const RECENT_TASKS_CAPACITY: usize = 10;

/// `/login` and `/tasks` registered together, the same shape `main.rs`
/// builds -- `/login` public, `/tasks` behind `crate::middleware::
/// require_auth` -- so these tests exercise both the login flow and the
/// middleware it feeds, end to end.
macro_rules! test_app {
    ($pool:expr) => {
        test::init_service(
            App::new()
                .app_data(web::Data::new($pool))
                .app_data(web::Data::new(common::test_app_config()))
                .app_data(web::Data::new(RecentTasksCache::new(RECENT_TASKS_CAPACITY)))
                .app_data(web::Data::new(common::test_audit_logger()))
                .app_data(web::Data::new(common::test_broadcaster()))
                .configure(routes::auth::configure)
                .configure(routes::tasks::configure),
        )
        .await
    };
}

#[actix_web::test]
async fn register_creates_a_user_and_returns_a_token_a_protected_route_accepts() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test_app!(pool);

    // No `insert_user` first -- registration is what creates the account.
    let request = test::TestRequest::post()
        .uri("/register")
        .set_json(json!({ "username": "newcomer", "password": "correct horse battery staple" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = test::read_body_json(response).await;
    let token = body["token"]
        .as_str()
        .expect("register response should include a token")
        .to_string();

    // The token /register just issued is accepted by a real protected route --
    // proof registration logs the new user straight in.
    let request = test::TestRequest::get()
        .uri("/tasks")
        .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn a_registered_user_can_then_log_in() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test_app!(pool);

    let register = test::TestRequest::post()
        .uri("/register")
        .set_json(json!({ "username": "returning", "password": "correct horse battery staple" }))
        .to_request();
    assert_eq!(
        test::call_service(&app, register).await.status(),
        StatusCode::CREATED
    );

    // The password /register hashed and stored is the one /login verifies
    // against -- one hashing implementation, both endpoints.
    let login = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "returning", "password": "correct horse battery staple" }))
        .to_request();
    let response = test::call_service(&app, login).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert!(body["token"].as_str().is_some());
}

#[actix_web::test]
async fn registering_a_taken_username_is_rejected_with_conflict() {
    let (pool, _container) = common::setup_test_db().await;
    let app = test_app!(pool);

    let first = test::TestRequest::post()
        .uri("/register")
        .set_json(json!({ "username": "dup", "password": "first password" }))
        .to_request();
    assert_eq!(
        test::call_service(&app, first).await.status(),
        StatusCode::CREATED
    );

    // Same username again -- the unique constraint on `users.username` becomes
    // a clean 409, not a 500.
    let second = test::TestRequest::post()
        .uri("/register")
        .set_json(json!({ "username": "dup", "password": "second password" }))
        .to_request();
    let response = test::call_service(&app, second).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"], "Username already taken");
}

#[actix_web::test]
async fn correct_credentials_return_a_token_that_a_protected_route_accepts() {
    let (pool, _container) = common::setup_test_db().await;
    common::insert_user(&pool, "alice", "correct horse battery staple").await;
    let app = test_app!(pool);

    let request = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "alice", "password": "correct horse battery staple" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    let token = body["token"]
        .as_str()
        .expect("login response should include a token")
        .to_string();

    // The token /login just issued should be accepted by a real protected
    // route -- proof the two halves of this module (login, middleware)
    // actually agree on the same secret and claim shape.
    let request = test::TestRequest::get()
        .uri("/tasks")
        .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn a_wrong_password_is_rejected() {
    let (pool, _container) = common::setup_test_db().await;
    common::insert_user(&pool, "alice", "correct horse battery staple").await;
    let app = test_app!(pool);

    let request = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "alice", "password": "wrong password" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn a_nonexistent_username_is_rejected_with_the_exact_same_response_as_a_wrong_password() {
    let (pool, _container) = common::setup_test_db().await;
    common::insert_user(&pool, "alice", "correct horse battery staple").await;
    let app = test_app!(pool);

    let wrong_password_request = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "alice", "password": "wrong password" }))
        .to_request();
    let wrong_password_response = test::call_service(&app, wrong_password_request).await;
    let wrong_password_status = wrong_password_response.status();
    let wrong_password_body: Value = test::read_body_json(wrong_password_response).await;

    let unknown_user_request = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "someone-who-does-not-exist", "password": "anything" }))
        .to_request();
    let unknown_user_response = test::call_service(&app, unknown_user_request).await;
    let unknown_user_status = unknown_user_response.status();
    let unknown_user_body: Value = test::read_body_json(unknown_user_response).await;

    // Same status, same body, for two very different failures -- see
    // `routes::auth::authenticate_user`'s doc comment on why leaking the
    // difference would let an attacker enumerate valid usernames.
    assert_eq!(wrong_password_status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_user_status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password_body, unknown_user_body);
}

#[actix_web::test]
async fn the_stored_password_hash_never_appears_in_the_login_response() {
    let (pool, _container) = common::setup_test_db().await;
    common::insert_user(&pool, "alice", "correct horse battery staple").await;
    let app = test_app!(pool);

    let request = test::TestRequest::post()
        .uri("/login")
        .set_json(json!({ "username": "alice", "password": "correct horse battery staple" }))
        .to_request();
    let response = test::call_service(&app, request).await;
    let body: Value = test::read_body_json(response).await;

    assert!(body.get("password_hash").is_none());
    assert!(body.get("password").is_none());
}
