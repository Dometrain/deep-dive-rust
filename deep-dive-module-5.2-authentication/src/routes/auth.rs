//! `/register` and `/login` -- Module 6's applied-for-real account endpoints,
//! tying together `crate::password` (hashing) and `crate::auth` (JWTs) into
//! real handlers. Both live deliberately outside the `/tasks` scope's
//! `require_auth` middleware (see `routes::tasks::configure`) -- a client
//! without a token yet is exactly who's supposed to be able to reach them.

use crate::{
    auth::create_jwt,
    config::AppConfig,
    db::DbPool,
    error::AppError,
    models::{LoginRequest, LoginResponse, RegisterRequest, User},
    password::{hash_password, verify_password},
};
use actix_web::{web, HttpResponse};
use tracing::instrument;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/register", web::post().to(register));
    cfg.route("/login", web::post().to(login));
}

/// Creates an account and logs the new user straight in -- it returns the same
/// `LoginResponse { token }` `/login` does, so a client is authenticated after
/// one call rather than having to `/register` and then `/login` separately.
/// A `201 Created` (not `200 OK`) says a new resource -- the user -- came into
/// existence. A duplicate username comes back as `409 Conflict`; see
/// [`create_user`].
#[instrument(name = "handler::register", skip_all, fields(username = %request.username))]
async fn register(
    pool: web::Data<DbPool>,
    config: web::Data<AppConfig>,
    request: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let user = create_user(pool.get_ref(), &request.username, &request.password).await?;

    let token = create_jwt(&user.id.to_string(), &config.jwt.secret);
    Ok(HttpResponse::Created().json(LoginResponse { token }))
}

/// Hashes `password` and inserts a new `users` row, returning the created
/// user. Reuses `password::hash_password` -- the *same* hashing `/login`'s
/// `verify_password` expects to check against later, never a second copy.
/// A unique-constraint violation on `username` (someone already registered
/// it) is translated into [`AppError::Conflict`] -- a `409`, not the generic
/// `500` a raw `sqlx::Error` would map to -- so the caller can tell "pick
/// another name" apart from a real database failure.
#[instrument(name = "create_user", skip_all, fields(username = %username))]
async fn create_user(pool: &DbPool, username: &str, password: &str) -> Result<User, AppError> {
    // A plain string password always hashes -- `hash_password` can only fail
    // for a pathological, effectively-unreachable input, so this matches the
    // crate's `.expect(...)`-for-unreachable idiom (see `auth::create_jwt`)
    // rather than inventing an error path for a case that can't happen here.
    let password_hash = hash_password(password).expect("a well-formed password always hashes");

    sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password_hash) VALUES ($1, $2) \
         RETURNING id, username, password_hash",
    )
    .bind(username)
    .bind(&password_hash)
    .fetch_one(pool.as_ref())
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
            AppError::Conflict("Username already taken".into())
        }
        _ => AppError::from(error),
    })
}

#[instrument(name = "handler::login", skip_all, fields(username = %request.username))]
async fn login(
    pool: web::Data<DbPool>,
    config: web::Data<AppConfig>,
    request: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    let user = authenticate_user(pool.get_ref(), &request.username, &request.password)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let token = create_jwt(&user.id.to_string(), &config.jwt.secret);
    Ok(HttpResponse::Ok().json(LoginResponse { token }))
}

/// Looks up `username` and checks `password` against its stored hash.
/// Returns `Ok(None)` -- not an error -- for both "no such user" and "wrong
/// password": [`login`] above turns either case into the exact same
/// `AppError::Unauthorized`, so a client can't tell which one failed from
/// the response alone. See this module's README on why that matters
/// (username enumeration).
#[instrument(name = "authenticate_user", skip_all, fields(username = %username))]
async fn authenticate_user(
    pool: &DbPool,
    username: &str,
    password: &str,
) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(pool.as_ref())
    .await?;

    let Some(user) = user else {
        return Ok(None);
    };

    Ok(verify_password(password, &user.password_hash).then_some(user))
}
