FROM rust:1.82-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release --bin web

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/web ./web
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static

ENV HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080

CMD ["./web"]
