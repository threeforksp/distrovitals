# Build stage
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install nightly toolchain for edition2024 support
RUN rustup toolchain install nightly && \
    rustup default nightly

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary with nightly
RUN cargo +nightly build --release --bin dv

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/dv /app/dv

# Copy web assets if they exist
COPY web ./web

# Expose port
EXPOSE 8080

# Run the server
# Bind to 0.0.0.0:8080 for Fly.io
CMD ["/app/dv", "serve", "--bind", "0.0.0.0:8080", "--static-dir", "web"]
