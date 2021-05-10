pub mod trash_dates;

use anyhow::Error;
use log::info;
use std::env;

async fn run() {
    info!("Starting bot.");
    use carapax::Api;

    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let api = Api::new(token).unwrap();

    let no_trash_message: String = String::from("No trash tomorrow!");

    use carapax::Dispatcher;

    let mut dispatcher = Dispatcher::new(api.clone());
    use carapax::{handler, types::Command, types::Message, HandlerResult};

    #[handler(command = "/start")]
    async fn command_handler(_context: &Api, _command: Command) {}

    dispatcher.add_handler(command_handler);

    let response = match trash_dates::get_tomorrows_trash().await {
        Ok(t) => match t[..] {
            [] => no_trash_message,
            _ => t.iter().map(ToString::to_string).collect(),
        },
        Err(_) => no_trash_message,
    };

    // while let Some(update) = stream.next().await {
    //     let update = update?;
    //     if let UpdateKind::Message(message) = update.kind {
    //         if let MessageKind::Text { ref data, .. } = message.kind {
    //             info!("<{}>: {}", &message.from.first_name, data);
    //
    //             let response = match trash_dates::get_tomorrows_trash().await {
    //                 Ok(t) => t.iter().map(ToString::to_string).collect(),
    //                 Err(_) => String::from("No trash tomorrow!"),
    //             };
    //             api.send(message.chat.text(response)).await?;
    //         }
    //     }
    // }
}

#[tokio::main]
async fn main() {
    run().await;
}
