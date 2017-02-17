FROM lawliet89/debian-rust:1.14.0
MAINTAINER Yong Wen Chua <me@yongwen.xyz>

RUN apt-get update \
    && apt-get install -y cmake

COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

COPY . ./
RUN cargo build --release

VOLUME /app/src/config

ENTRYPOINT ["cargo"]
CMD ["run", "--release", "--", "/app/src/config/config.json"]
