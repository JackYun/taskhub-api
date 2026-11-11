# syntax=docker/dockerfile:1

# ---- Build stage -------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Cache dependencies: build a dummy crate with the real manifest first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

# Build the real application (migrations are embedded at compile time).
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked

# ---- Runtime stage -----------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 taskhub \
    && useradd --system --uid 10001 --gid taskhub --home-dir /app --shell /usr/sbin/nologin taskhub \
    && mkdir -p /app/data \
    && chown -R taskhub:taskhub /app

WORKDIR /app
COPY --from=builder /app/target/release/taskhub-api /usr/local/bin/taskhub-api

ENV HOST=0.0.0.0 \
    PORT=3000 \
    DATABASE_URL=sqlite:///app/data/taskhub.db \
    RUST_LOG=taskhub_api=info,tower_http=info
# JWT_SECRET has no default on purpose: release builds refuse to start without it.

VOLUME ["/app/data"]
EXPOSE 3000
USER taskhub

CMD ["taskhub-api"]
