FROM rust:latest as builder
WORKDIR /usr/src/telegram_notificator

COPY src src
COPY graphql graphql
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY trash_bot.iml trash_bot.iml

RUN cargo install --debug --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/trash_bot /usr/local/bin/trash_bot
CMD ["trash_bot"]
