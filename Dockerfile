FROM rust:latest

# Install Typst CLI
RUN curl -L https://github.com/typst/typst/releases/latest/download/typst-x86_64-unknown-linux-gnu.tar.gz \
    | tar -xz -C /usr/local/bin

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

COPY . .

RUN cargo build --release

CMD ["./target/release/typst-server"]
