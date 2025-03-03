FROM rust:latest

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

COPY . .

RUN cargo build --release

CMD ["./target/release/typst-server"]
