# =============================================================================
# Stage 1: Tool Cache - Build/install cargo tools once and cache them
# =============================================================================
FROM rustlang/rust:nightly-alpine AS tool-cache

RUN apk add --no-cache libc-dev openssl-dev sqlite-dev perl make

# Install cargo tools in a dedicated stage for better caching
RUN cargo install sqlx-cli --no-default-features --features sqlite --locked
RUN cargo install cargo-leptos --locked

# =============================================================================
# Stage 2: Builder - Build the Leptos SSR application
# =============================================================================
FROM rustlang/rust:nightly-alpine AS builder

# Install build dependencies
RUN apk update && \
  apk add --no-cache \
  bash \
  curl \
  npm \
  libc-dev \
  binaryen \
  openssl-dev \
  openssl-libs-static \
  pkgconfig \
  sqlite \
  sqlite-dev \
  musl-dev

# Copy pre-built tools from cache stage
COPY --from=tool-cache /usr/local/cargo/bin/sqlx /usr/local/cargo/bin/sqlx
COPY --from=tool-cache /usr/local/cargo/bin/cargo-sqlx /usr/local/cargo/bin/cargo-sqlx
COPY --from=tool-cache /usr/local/cargo/bin/cargo-leptos /usr/local/cargo/bin/cargo-leptos

# Add WASM target for client-side hydration
RUN rustup target add wasm32-unknown-unknown

# Set working directory
WORKDIR /work

# Copy dependency manifests first and create dummy src for dependency caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && \
  echo "fn main() {}" > src/main.rs && \
  echo "pub fn dummy() {}" > src/lib.rs
RUN cargo build --release --features ssr && rm -rf src target/release/deps/flashy*

# Now copy real source code
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
# Stage 3: Runtime Dependencies - Build minimal sqlx for runtime
# =============================================================================
FROM rustlang/rust:nightly-alpine AS runtime-tools

RUN apk add --no-cache libc-dev openssl-dev sqlite-dev musl-dev

# Build sqlx statically for distroless
RUN cargo install sqlx-cli --no-default-features --features sqlite --locked \
  --target x86_64-unknown-linux-musl || \
  cargo install sqlx-cli --no-default-features --features sqlite --locked

# =============================================================================
# Stage 4: Runner - Distroless minimal runtime image
# =============================================================================
# :debug needed for busybox shell for entrypoint script
FROM gcr.io/distroless/cc-debian12:debug

WORKDIR /app

COPY --from=builder --chmod=755 /work/target/release/flashy /app/flashy
COPY --from=builder --chmod=644 /work/Cargo.toml /app/Cargo.toml

COPY --from=builder /work/target/site /app/site
COPY --from=builder /work/migrations /app/migrations

COPY --from=runtime-tools --chmod=755 /usr/local/cargo/bin/sqlx /app/sqlx

COPY --chmod=755 docker-entrypoint.sh /app/docker-entrypoint.sh

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="/app/site"
ENV DATABASE_URL="sqlite:/app/data/flashy.db"

EXPOSE 8080

VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["/busybox/wget", "-q", "-O", "/dev/null", "http://localhost:8080/"]

ENTRYPOINT ["/busybox/sh", "/app/docker-entrypoint.sh"]
