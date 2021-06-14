FROM rust:latest as builder
WORKDIR /usr/src/telegram_notificator

COPY src src
COPY graphql graphql
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY trash_bot.iml trash_bot.iml

RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/telegram_notificator /usr/local/bin/telegram_notificator
CMD ["trash_bot"]
