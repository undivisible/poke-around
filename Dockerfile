FROM oven/bun:latest AS builder-bun
WORKDIR /app
COPY . .
RUN bun install --cwd bridge && bun run build:bridge

FROM debian:bookworm-slim AS builder-zig
WORKDIR /app
RUN apt-get update && apt-get install -y curl xz-utils && \
    curl -sfL https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz | tar xJ && \
    mv zig-x86_64-linux-0.15.2/zig /usr/local/bin/zig
COPY --from=builder-bun /app .
RUN zig build -Doptimize=ReleaseSafe

FROM node:22-slim
COPY --from=builder-zig /app/zig-out/bin/poke-around /usr/local/bin/poke-around
COPY --from=builder-zig /app/bridge/dist/poke-around-bridge.js /usr/local/bin/poke-around-bridge.js
ENTRYPOINT ["poke-around"]
