pub mod trash_dates;

use anyhow::Error;
use log::info;
use std::env;
use teloxide::{prelude::*, utils::command::BotCommand};

#[derive(BotCommand)]
#[command(rename = "lowercase", description = "Welcome to the Trash-Bot!")]
enum Command {
    #[command(description = "Check tomorrows trash dates.")]
    Trash,
    #[command(description = "Display this text.")]
    Help,
}

async fn answer(cx: UpdateWithCx<AutoSend<Bot>, Message>, command: Command) -> Result<(), Error> {
    let no_trash_message = String::from("No trash tomorrow!");

    match command {
        Command::Trash => {
            let response = match trash_dates::get_tomorrows_trash().await {
                Ok(t) => match t[..] {
                    [] => no_trash_message,
                    _ => t.iter().map(ToString::to_string).collect(),
                },
                Err(_) => no_trash_message,
            };
            cx.answer(response).await?;
        }
        Command::Help => cx.answer(Command::desctiptions()).await?,
    }

    Ok(())
}

async fn run() {
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    teloxide::enable_logging!();

    info!("Starting bot.");

    let bot = teloxide::prelude::Bot::new(token).auto_send();
    let bot_name: String = String::from("fluxinator");

    teloxide::commands_repl(bot, bot_name, answer).await;

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
