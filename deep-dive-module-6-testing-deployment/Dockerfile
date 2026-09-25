# `cargo-chef` splits the build so that dependencies land in their own cached
# layer. Without it, `COPY src` comes before `cargo build`, so editing a single
# line of app code invalidates the layer and every crate in the graph (actix,
# sqlx, tonic, opentelemetry, ...) recompiles from scratch. With it, only a
# change to `Cargo.toml`/`Cargo.lock` recompiles dependencies.
FROM lukemathwalker/cargo-chef:latest-rust-1.88-bookworm AS chef
WORKDIR /app

# Stage 1: distil the manifests into `recipe.json`, a "skeleton" of the project
# that describes its dependencies and nothing else. This stage re-runs on every
# source change, but it is cheap, and its output only changes when the
# dependency graph does. `shared` is copied because it's a path dependency
# (dev-only, but Cargo still resolves it against `Cargo.lock`).
FROM chef AS planner
COPY Cargo.toml Cargo.lock build.rs ./
COPY shared ./shared
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: build only the dependencies from the recipe. The `COPY` of
# `recipe.json` is the cache key: as long as it's byte-identical, Docker reuses
# this layer and skips the slow compile entirely. The flags must match the real
# build below (`--release`, `--locked`), or cargo will treat the cached
# artifacts as a different build and recompile them. `shared` has to be copied
# in for real: it's a path dependency outside any workspace, so the recipe
# only references it rather than carrying a skeleton of it. It's a tiny crate
# that rarely changes, so it barely dents the cache.
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY shared ./shared
RUN cargo chef cook --release --locked --recipe-path recipe.json

# Stage 3: build the app itself against the pre-built dependencies.
# `COPY native` is still needed here: `build.rs`'s `cc::Build` call compiles
# `native/native.c`, and without it the build fails before any Rust compiles.
COPY Cargo.toml Cargo.lock build.rs ./
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
