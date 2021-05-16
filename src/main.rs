use std::env;

use anyhow::Error;
use carapax::methods::SendMessage;
use carapax::Api;
use log::info;
use std::fmt::Display;
use std::future::Future;
use std::ops::{Index, RangeBounds, RangeFull};
use std::process::Output;
use std::slice::SliceIndex;
use tokio::time::Duration;
use trash_bot::trash_dates::TrashDate;

pub mod trash_dates;

async fn get_message<T>(request_future: impl Future<Output = Result<Vec<T>, Error>>) -> String
where
    T: ToString,
{
    let no_trash_message: String = String::from("No trash tomorrow!");

    match request_future.await {
        Ok(t) => {
            if t.is_empty() {
                no_trash_message
            } else {
                t.iter().map(ToString::to_string).collect()
            }
        }
        Err(_) => no_trash_message,
    }
}

async fn get_users() {}

async fn perform_tasks() {
    let request_performer = trash_dates::RequestPerformer::from_env();

    let users: Vec<trash_dates::User> = match request_performer.get_active_users().await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("No users with active notifications found: {}", e);
            return;
        }
    };

    for user in users {
        tokio::spawn(async move {
            let request_performer = trash_dates::RequestPerformer::from_env();

            let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
            let api: Api = Api::new(token).unwrap();

            let future_result = request_performer.get_tomorrows_trash();
            api.execute(SendMessage::new(
                user.client_id,
                get_message(future_result).await,
            ))
            .await
            .unwrap();
        });
    }
}

async fn run() {
    info!("Starting bot.");

    use clokwerk::{AsyncScheduler, Job, TimeUnits};

    let mut scheduler = AsyncScheduler::with_tz(chrono_tz::Europe::Berlin);

    perform_tasks().await;
    // scheduler.every(1.day()).at("16:00:00").run(perform_tasks);
    //
    // loop {
    //     scheduler.run_pending().await;
    //     tokio::time::sleep(Duration::from_secs(10)).await;
    // }

    // REst

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
