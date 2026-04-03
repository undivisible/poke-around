FROM oven/bun:latest AS builder-bun
WORKDIR /app
COPY . .
RUN bun install --cwd bridge && bun run build:bridge

FROM ghcr.io/ziglang/zig:0.15.2 AS builder-zig
WORKDIR /app
COPY --from=builder-bun /app .
RUN zig build -Doptimize=ReleaseSafe

FROM node:22-slim
COPY --from=builder-zig /app/zig-out/bin/poke-around /usr/local/bin/poke-around
COPY --from=builder-zig /app/bridge/dist/poke-around-bridge.js /usr/local/bin/poke-around-bridge.js
ENTRYPOINT ["poke-around"]
