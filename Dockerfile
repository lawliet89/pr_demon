FROM lawliet89/docker-rust:1.18.0 as builder

ARG ARCHITECTURE=x86_64-unknown-linux-musl
WORKDIR /app/src
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked -v

COPY ./ ./
RUN cargo build --release --target "${ARCHITECTURE}" -v --frozen

# Runtime Image

FROM alpine:3.5
ARG ARCHITECTURE=x86_64-unknown-linux-musl
WORKDIR /app
COPY --from=builder /app/src/target/${ARCHITECTURE}/release/pr_demon .
CMD ["/app/pr_demon", "/app/src/config/config.yml"]
