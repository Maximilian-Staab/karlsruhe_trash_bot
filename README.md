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
| OPENSTREETMAP_ENDPOINT | https://nominatim.openstreetmap.org/ | (Optional) proxy for caching requests |
| RUST_LOG           |         | (Optional) Set log level for the application |


# TODO:

* [ ] Add doc-tests
* [ ] Convert geocoding crate to be non-blocking, then use it here
* [ ] Add more cities?


## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
