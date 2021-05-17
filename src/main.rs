use std::env;

use carapax::methods::SendMessage;
use carapax::Api;
use log::info;
use tokio::time::Duration;

use trash_bot::trash_dates::{RequestPerformer, User};

pub mod trash_dates;

async fn get_message<T>(something: &[T]) -> String
where
    T: ToString,
{
    if let [] = something {
        "No trash tomorrow!".to_string()
    } else {
        something.iter().map(ToString::to_string).collect()
    }
}

async fn perform_tasks() {
    let request_performer = RequestPerformer::from_env();

    let users: Vec<User> = match request_performer.get_active_users_tomorrow().await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("No users with active notifications found: {}", e);
            return;
        }
    };

    for user in users {
        tokio::spawn(async move {
            let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
            let api: Api = Api::new(token).unwrap();

            api.execute(SendMessage::new(
                user.client_id,
                get_message(&user.dates[..]).await,
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

    scheduler.every(1.day()).at("16:00:00").run(perform_tasks);

    loop {
        scheduler.run_pending().await;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    // Rest

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
