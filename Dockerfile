# Build stage
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Cache dependency build
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs && cargo build --release && rm -rf src
COPY . .
RUN touch src/main.rs && cargo build --release

# Runtime stage — distroless for minimal attack surface
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /app/target/release/pavilion /app/pavilion
COPY --from=builder /app/static /app/static
COPY --from=builder /app/db /app/db
COPY --from=builder /app/templates /app/templates
WORKDIR /app
EXPOSE 3000
ENTRYPOINT ["/app/pavilion"]
