# ── Stage 1: Build ────────────────────────────────────────────────────────────
#
# Builds both the WASM frontend (dx build) and the Axum server binary.
# Uses a cache mount for the Cargo registry so repeated builds are fast.
FROM rust:1.88-slim AS builder

# System deps: pkg-config + libssl for any TLS crates; also needed by sqlx
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add the WASM compile target
RUN rustup target add wasm32-unknown-unknown

# Install Dioxus CLI (pinned to match the dioxus crate version in Cargo.toml)
RUN cargo install dioxus-cli --version "0.6.3" --locked

WORKDIR /app

# Copy the full source tree
COPY . .

# Build the WASM frontend — output goes to ./dist/
RUN dx build --platform web --release

# Build the Axum server binary (disable default 'web' feature — wasm deps don't compile on native)
RUN cargo build --bin server --no-default-features --features server --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
#
# Slim Debian image — only the server binary + the compiled frontend assets.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the server binary
COPY --from=builder /app/target/release/server ./server

# Copy the compiled WASM frontend that the server will serve statically
# Note: path is target/dx/<app>/release/web/public (Dioxus 0.6 output layout)
COPY --from=builder /app/target/dx/chord_shifter/release/web/public ./dist

# The persistent SQLite volume will be mounted at /data by Fly.io
RUN mkdir -p /data

EXPOSE 8080

CMD ["./server"]
