# Build stage
FROM rust:latest AS builder
WORKDIR /app

# Copy source code and build
COPY . .
# RUN cargo build --release
RUN cargo build

RUN ls -la /app/target/release

# Runtime stage
FROM alpine:latest
WORKDIR /app

# Install dependencies if needed (glibc compatibility, etc.)
# RUN apk add --no-cache libc6-compat

# Copy the built binary
COPY --from=builder /app/target/release/typst-server /usr/local/bin/typst-server

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/typst-server"]
