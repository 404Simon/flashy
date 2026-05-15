# =============================================================================
# Stage 1: Tool Cache - Build/install cargo tools once and cache them
# =============================================================================
FROM rustlang/rust:nightly-slim AS tool-cache

RUN apt-get update && apt-get install -y --no-install-recommends \
  curl ca-certificates pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

RUN curl -L --proto '=https' --tlsv1.2 -sSf \
  https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
  | bash

# Install cargo-leptos from pre-built binaries
RUN cargo binstall cargo-leptos --no-confirm --locked
RUN cargo install sqlx-cli --no-default-features --features sqlite,rustls --locked

# =============================================================================
# Stage 2: Builder - Build the Leptos SSR application
# =============================================================================
FROM rustlang/rust:nightly-slim AS builder

RUN apt-get update && \
  apt-get install -y --no-install-recommends curl ca-certificates && \
  curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && \
  apt-get install -y --no-install-recommends \
  bash \
  nodejs \
  build-essential \
  binaryen \
  pkg-config \
  libssl-dev \
  libsqlite3-dev \
  && rm -rf /var/lib/apt/lists/*

COPY --from=tool-cache /usr/local/cargo/bin/sqlx /usr/local/cargo/bin/sqlx
COPY --from=tool-cache /usr/local/cargo/bin/cargo-sqlx /usr/local/cargo/bin/cargo-sqlx
COPY --from=tool-cache /usr/local/cargo/bin/cargo-leptos /usr/local/cargo/bin/cargo-leptos

RUN rustup target add wasm32-unknown-unknown

WORKDIR /work

COPY package.json pnpm-lock.yaml ./
RUN corepack enable pnpm && pnpm install --frozen-lockfile

# create dummy src for dependency caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && \
  echo "fn main() {}" > src/main.rs && \
  echo "pub fn dummy() {}" > src/lib.rs
RUN cargo build --release --features ssr && rm -rf src target/release/deps/flashy*

# now copy real source code
COPY src ./src
COPY style ./style
COPY public ./public
COPY migrations ./migrations

ENV DATABASE_URL=sqlite:build.db
RUN sqlx database create && \
  sqlx migrate run && \
  cargo sqlx prepare --workspace -- --lib --features ssr && \
  rm build.db*

# Set SQLx to offline mode for the actual build
ENV SQLX_OFFLINE=true

# Build the application in release mode with optimizations
ENV CARGO_PROFILE_RELEASE_STRIP=symbols
ENV CARGO_PROFILE_RELEASE_LTO=true
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
RUN cargo leptos build --release -vv

# =============================================================================
# Stage 3: Runner - Minimal Debian slim image with runtime dependencies
# =============================================================================
FROM debian:13-slim

RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  ca-certificates \
  poppler-utils \
  wget \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder --chmod=755 /work/target/release/flashy /app/flashy
COPY --from=builder --chmod=644 /work/Cargo.toml /app/Cargo.toml

COPY --from=builder /work/target/site /app/site
COPY --from=builder /work/migrations /app/migrations

COPY --from=tool-cache --chmod=755 /usr/local/cargo/bin/sqlx /app/sqlx

COPY --chmod=755 docker-entrypoint.sh /app/docker-entrypoint.sh

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="/app/site"
ENV DATABASE_URL="sqlite:/app/data/flashy.db"

EXPOSE 8080

VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["wget", "-q", "-O", "/dev/null", "http://localhost:8080/"]

ENTRYPOINT ["/bin/sh", "/app/docker-entrypoint.sh"]
