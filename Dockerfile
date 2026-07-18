# =============================================================================
# Stage 1: Builder - Build the Leptos SSR application
# =============================================================================
FROM rust:1-slim-trixie AS builder

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

RUN curl -LO https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-gnu.tgz && \
  tar -xvf cargo-binstall-x86_64-unknown-linux-gnu.tgz && \
  cp cargo-binstall /usr/local/cargo/bin && \
  rm -rf cargo-binstall cargo-binstall-x86_64-unknown-linux-gnu.tgz

RUN cargo binstall cargo-leptos sqlx-cli --no-confirm --locked

RUN rustup target add wasm32-unknown-unknown

WORKDIR /work

COPY package.json pnpm-lock.yaml ./
RUN corepack enable pnpm && pnpm install --frozen-lockfile

# create dummy src for dependency caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && \
  echo "fn main() { flashy::main() }" > src/main.rs && \
  echo "use leptos::*; pub fn main() {}" > src/lib.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/work/target \
  cargo build --release --features ssr && rm -rf src target/release/deps/flashy*

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
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/work/target \
  cargo leptos build --release -vv && \
  cp target/release/flashy /work/flashy && \
  cp -r target/site /work/site

# =============================================================================
# Stage 3: Runner - Distroless minimal runtime image
# =============================================================================
# :debug needed for busybox shell for entrypoint script
FROM gcr.io/distroless/cc-debian13:debug

WORKDIR /app

COPY --from=builder --chmod=755 /work/flashy /app/flashy
COPY --from=builder --chmod=644 /work/Cargo.toml /app/Cargo.toml

COPY --from=builder /work/site /app/site
COPY --from=builder /work/migrations /app/migrations

COPY --from=builder --chmod=755 /usr/local/cargo/bin/sqlx /app/sqlx

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
