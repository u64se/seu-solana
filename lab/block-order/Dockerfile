# Stage 1: Compile pure-Rust u64se-block-order (`block-order-lab`) directly from .rs source
FROM rust:latest AS builder

WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy core u64se-seu-solana library and lab/block-order crate
COPY Cargo.toml ./
COPY src ./src
COPY lab/block-order ./lab/block-order

WORKDIR /build/lab/block-order
RUN cargo build --release --target-dir /build/target

# Stage 2: Minimal reproducible runtime container
FROM debian:bookworm-slim

WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/block-order-lab /usr/local/bin/block-order-lab

ENV SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"
ENV SOLANA_KEYPAIR_PATH="/secrets/id.json"
ENV TELEMETRY_DIR="/app/telemetry"

ENTRYPOINT ["block-order-lab"]
CMD ["--help"]
