FROM ekidd/rust-musl-builder:latest as builder
WORKDIR /usr/src/telegram_notificator

USER root

COPY src src
COPY graphql graphql
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY trash_bot.iml trash_bot.iml

RUN rustup toolchain list
RUN rustup target list
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM alpine:latest
RUN apk add libgcc
COPY --from=builder /usr/src/telegram_notificator/target/x86_64-unknown-linux-musl/release/trash_bot /usr/local/bin/

CMD ["usr/local/bin/trash_bot"]
