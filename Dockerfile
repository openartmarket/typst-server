# Build stage
FROM rust:latest AS builder
WORKDIR /app

# Copy source code and build
COPY . .
RUN cargo build --release

RUN ls -la /app/target/release

# Runtime stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install OpenSSL 3
RUN apt-get update && apt-get install -y libssl3 && ldconfig

# Copy the built binary
COPY --from=builder /app/target/release/typst-server /usr/local/bin/typst-server

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/typst-server"]
