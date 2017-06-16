FROM lawliet89/docker-rust:1.18.0 as builder

WORKDIR /app/src
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked -v

COPY ./ ./
RUN cargo build --release --target "x86_64-unknown-linux-musl" -v --frozen

CMD ["/app/src/target/x86_64-unknown-linux-musl/release/pr_demon", "/app/src/config/config.yml"]
