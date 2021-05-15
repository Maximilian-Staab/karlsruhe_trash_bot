use std::env;

use carapax::Api;
use carapax::methods::SendMessage;
use log::info;
use tokio::time::Duration;

pub mod trash_dates;

// static TOKEN: &str = "489706166:AAH2D80-MdEmwKiUIfUXty1L3bhQsf15Wbc";
// static TOKEN: String = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
// static API: Api = Api::new(TOKEN).unwrap();

// async fn send_message(api: &'static Api, chat_id: Integer) {
//     let mut scheduler = AsyncScheduler::with_tz(chrono_tz::Europe::Berlin);
//     scheduler
//         .every(1.minutes())
//         .run(|| async { api.execute(SendMessage::new(chat_id, get_message().await)); });
// }

async fn get_message() -> String {
    let no_trash_message: String = String::from("No trash tomorrow!");

    match trash_dates::get_tomorrows_trash().await {
        Ok(t) => match t[..] {
            [] => no_trash_message,
            _ => t.iter().map(ToString::to_string).collect(),
        },
        Err(_) => no_trash_message,
    }
}

async fn run() {
    info!("Starting bot.");

    use clokwerk::{AsyncScheduler, Job, TimeUnits};

    // use carapax::Dispatcher;

    // send_message(&API, 385363028);
    // let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    // let api: Api = Api::new(token).unwrap();

    let mut scheduler = AsyncScheduler::with_tz(chrono_tz::Europe::Berlin);
    scheduler.every(1.day()).at("16:00:00").run(|| async {
        let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
        let api: Api = Api::new(token).unwrap();
        api.execute(SendMessage::new(385363028, get_message().await))
            .await
            .unwrap();
    });

    loop {
        scheduler.run_pending().await;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    // let mut dispatcher = Dispatcher::new(api.clone());
    // use carapax::{handler, types::Command, types::Message, HandlerResult};

    // #[handler(command = "/start")]
    // async fn command_handler(_context: &Api, _command: Command) {
    //     info!(_command)
    // }

    // dispatcher.add_handler(command_handler);

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
