FROM rust:1.88-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY native ./native
COPY src ./src
COPY migrations ./migrations
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
# `ca-certificates` is the only package the *app* itself needs -- `sqlx`'s
# Postgres driver is pure Rust, so there's no `libpq` to install, despite what
# many Rust-on-Postgres tutorials show. `curl` is added for one reason only:
# the container healthcheck in `docker-compose.yml` polls `/health` from
# *inside* the container, and the slim base ships no HTTP client. The app
# never calls curl itself.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home appuser

WORKDIR /app
COPY --from=builder /app/target/release/todo-ws-app /usr/local/bin/todo-ws-app
COPY config ./config
COPY migrations ./migrations

ENV APP_SERVER_HOST=0.0.0.0 \
    APP_SERVER_PORT=8080 \
    APP_SERVER_LOG_LEVEL=info \
    APP_DATABASE_URL=postgres://postgres:postgres@db:5432/todo \
    APP_JWT_SECRET=dev-only-secret-change-me \
    RUST_BACKTRACE=1

USER appuser
EXPOSE 8080
CMD ["todo-ws-app"]
