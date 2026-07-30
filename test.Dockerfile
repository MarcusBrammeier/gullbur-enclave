FROM docker.io/rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p gullbur-cli --features headless

FROM docker.io/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/gullbur-cli /usr/local/bin/
EXPOSE 19876 19877 19878
ENTRYPOINT ["gullbur-cli"]