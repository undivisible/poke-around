FROM oven/bun:latest AS builder-bun
WORKDIR /app
COPY . .
RUN bun install --cwd bridge && bun run build:bridge

FROM rust:1-bookworm AS builder-rust
WORKDIR /app
COPY --from=builder-bun /app .
RUN cargo build --workspace --release

FROM node:22-slim
COPY --from=builder-rust /app/target/release/poke-around /usr/local/bin/poke-around
COPY --from=builder-rust /app/bridge/dist/poke-around-bridge.js /usr/local/bin/poke-around-bridge.js
ENTRYPOINT ["poke-around"]
