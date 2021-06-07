use std::env;
use std::future::Future;

use crate::location_lookup::Lookup;
use anyhow::Error;
use carapax::methods::SendMessage;
use carapax::types::{KeyboardButton, ReplyKeyboardMarkup, ReplyMarkup};
use carapax::Api;
use geocoding::openstreetmap::AddressDetails;
use location_lookup::LocationLookup;
use log::info;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use trash_bot::trash_dates::{RequestPerformer, User};

mod location_lookup;
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

async fn perform_tasks() {
    let request_performer = RequestPerformer::from_env();

    let users: Vec<User> = match request_performer.get_active_users().await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("No users with active notifications found: {}", e);
            return;
        }
    };

    for user in users {
        tokio::spawn(async move {
            let request_performer = RequestPerformer::from_env();

            let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
            let api: Api = Api::new(token).unwrap();

            let user_id = user.client_id;
            let future_result = request_performer.get_tomorrows_trash(user_id);
            api.execute(SendMessage::new(user_id, get_message(future_result).await))
                .await
                .unwrap();
        });
    }
}

struct SenderContext {
    pub api: Api,
    pub sender: mpsc::Sender<Lookup>,
}

async fn run() {
    info!("Starting bot.");

    use clokwerk::{AsyncScheduler, Job, TimeUnits};

    let chat_id = env::var("TELEGRAM_CHAT_ID").expect("CHAT id is missing");
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let api: Api = Api::new(token).unwrap();

    let (tx, rx) = mpsc::channel::<Lookup>(32);

    let mut lookup_device = LocationLookup::new(rx).await;

    tokio::spawn(async move {
        lookup_device.start().await;
    });

    // let mut scheduler = AsyncScheduler::with_tz(chrono_tz::Europe::Berlin);
    //
    // scheduler.every(1.day()).at("16:00:00").run(perform_tasks);
    //
    // loop {
    //     scheduler.run_pending().await;
    //     tokio::time::sleep(Duration::from_secs(10)).await;
    // }

    // REst

    let row = vec![
        vec![KeyboardButton::new("Test")],
        vec![KeyboardButton::new("one")],
        vec![KeyboardButton::new("two")],
        vec![KeyboardButton::new("last")],
        vec![KeyboardButton::new("location").request_location()],
    ];

    let markup = ReplyKeyboardMarkup::from(row.clone())
        .one_time_keyboard(false)
        .resize_keyboard(false);

    let replymarkup = ReplyMarkup::from(markup);

    let message = SendMessage::new(chat_id, "message text");
    api.execute(message.reply_markup(replymarkup))
        .await
        .expect("Something went wrong");

    let sender_context = SenderContext {
        api: api.clone(),
        sender: tx.clone(),
    };

    use carapax::{
        handler, types::Command, types::Message, types::MessageKind, Dispatcher, HandlerResult,
    };
    let mut dispatcher = Dispatcher::new(sender_context);

    #[handler]
    async fn message_handler(_context: &SenderContext, _message: Message) -> HandlerResult {
        let message_string = match _message.get_text() {
            Some(t) => &t.data,
            None => "<nothing",
        };

        let message_data = &_message.data;
        if let carapax::types::MessageData::Location(location) = message_data {
            let location = *location;

            let (tx, rx) = oneshot::channel::<Result<Option<AddressDetails>, Error>>();

            let send_result = _context
                .sender
                .send(Lookup {
                    longitude: location.longitude,
                    latitude: location.latitude,
                    responder: tx,
                })
                .await;
            match send_result {
                Ok(_) => {
                    let result = match rx.await {
                        Ok(t) => t,
                        Err(e) => {
                            log::error!("Some kind of error: {}", e);
                            return HandlerResult::Continue;
                        }
                    };

                    let result = match result {
                        Ok(t) => t,
                        Err(e) => {
                            log::error!("Another error: {:?}", e);
                            return HandlerResult::Continue;
                        }
                    };

                    _context
                        .api
                        .execute(SendMessage::new(
                            _message.get_chat_id(),
                            format!("Is this your location? {:?}", result),
                        ))
                        .await
                        .unwrap();
                }
                Err(e) => {
                    log::error!("Send Error or what: {}", e);
                }
            }

            return HandlerResult::Continue;
        }

        println!("{:?}", message_data);

        _context
            .api
            .execute(SendMessage::new(_message.get_chat_id(), message_string))
            .await
            .unwrap();

        HandlerResult::Continue
    }

    dispatcher.add_handler(message_handler);

    // while let Some(update) = stream.next().await {
    //     let update = update?;
    //     if let UpdateKind::Message(message) = update.kind {
    //         if let MessageKind::Text { ref data, .. } = message.kind {
    //             info!("<{}>: {}", &message.from.first_name, data);
    //
    //             let response = String::from("Hallo");
    //             api.send(message.chat.text(response)).await?;
    //         }
    //     }
    // }
    carapax::longpoll::LongPoll::new(api, dispatcher)
        .run()
        .await;
}

#[tokio::main]
async fn main() {
    env_logger::init();
    run().await;
}
