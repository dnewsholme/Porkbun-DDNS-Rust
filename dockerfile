FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /svc
COPY --from=builder /app/target/release/porkbun_ddns .
ENV RUST_LOG=info
CMD ["./porkbun_ddns"]