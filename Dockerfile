# Stage 1: Build Zig binary and Bun bridge
FROM oven/bun:latest as builder-bun
WORKDIR /app
COPY . .
RUN bun run build:bridge

FROM mlugg/zig:0.13.0 as builder-zig
WORKDIR /app
COPY --from=builder-bun /app .
RUN zig build -Doptimize=ReleaseSafe

# Stage 2: Final runtime image
FROM node:20-slim
WORKDIR /app
COPY --from=builder-zig /app/zig-out/bin/poke-gate /usr/local/bin/poke-gate
COPY --from=builder-zig /app/bridge/dist/poke-gate-bridge.js /usr/local/bin/poke-gate-bridge.js

# Note: The Zig app expects the bridge to be alongside it or in certain paths.
# Based on src/app.zig, it looks for it alongside the executable.

ENTRYPOINT ["poke-gate"]
