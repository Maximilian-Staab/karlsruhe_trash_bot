[![Publish Docker image](https://github.com/Maximilian-Staab/karlsruhe_trash_bot/actions/workflows/main.yml/badge.svg?branch=main)](https://github.com/Maximilian-Staab/karlsruhe_trash_bot/actions/workflows/main.yml)

# Entsorgungskalender Chatbot - Karlsruhe 

**Dieser Chatbot hat nichts mit irgendeiner offiziellen Stelle der Stadt Karlsruhe zu tun. Dies ist ein Hobbyprojekt,
welches ich fur mich selbst geschrieben habe.**

Mit diesem Telegram-Bot kann man sich die Entsorgungstermine in Karlsruhe zuschicken lassen. Dazu speichert man eine
Strassen und Hausnummer, zu der man Nachrichten bekommen mochte. Anschließend wird einem um 16 Uhr eine Nachricht
geschickt, wenn am na echten Tag Bio/Papier/Restmüll abgeholt wird.

# Environment Variables

A graphql api is used for interacting with the database. These are the required environment variables used to connect
and authenticate with it:

| Key                | Default | Description                                  |
| ------------------ | :-----: | -------------------------------------------- |
| TELEGRAM_BOT_TOKEN |         | Token for your telegram bot                  |
| HASURA_ENDPOINT    |         | Graphql endpoint url                         |
| HASURA_SECRET      |         | Graphql endpoint secret                      |
| RUST_LOG           |         | (Optional) Set log level for the application |


# TODO:

* [ ] Add doc-tests
* [ ] Convert geocoding crate to be non-blocking, then use it here
* [ ] Add more cities?
