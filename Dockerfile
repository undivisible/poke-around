FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --workspace --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin poke
COPY --from=builder /app/target/release/poke-around /usr/local/bin/poke-around
USER poke
ENTRYPOINT ["poke-around"]