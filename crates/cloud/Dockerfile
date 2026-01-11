# Enya Cloud - Multi-stage build
# Optimized for small final image size

# =============================================================================
# Build stage
# =============================================================================
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/cloud/Cargo.toml crates/cloud/Cargo.toml
COPY crates/team-api/Cargo.toml crates/team-api/Cargo.toml

# Create dummy source files to cache dependencies
RUN mkdir -p crates/cloud/src crates/team-api/src && \
    echo "fn main() {}" > crates/cloud/src/main.rs && \
    echo "pub fn dummy() {}" > crates/team-api/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release -p enya-cloud 2>/dev/null || true

# Copy actual source code
COPY crates/cloud crates/cloud
COPY crates/team-api crates/team-api

# Touch source files to invalidate cache
RUN touch crates/cloud/src/main.rs crates/team-api/src/lib.rs

# Build the actual binary
RUN cargo build --release -p enya-cloud

# =============================================================================
# Runtime stage
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash enya

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/enya-cloud /app/enya-cloud

# Copy migrations
COPY --from=builder /app/crates/cloud/migrations /app/migrations

# Set ownership
RUN chown -R enya:enya /app

USER enya

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the binary
CMD ["/app/enya-cloud"]
