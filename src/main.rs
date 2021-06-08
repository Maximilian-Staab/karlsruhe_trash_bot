use std::env;

use crate::bot_logic::Bot;
use crate::location_lookup::{LocationResult, Lookup};
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

mod bot_logic;
mod location_lookup;
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
            let request_performer = RequestPerformer::from_env();

            let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
            let api: Api = Api::new(token).unwrap();

            let user_id = user.client_id;
            match request_performer.get_tomorrows_trash(user_id).await {
                Ok(t) => {
                    api.execute(SendMessage::new(user_id, get_message(&t).await))
                        .await
                        .unwrap();
                }
                Err(e) => log::error!(
                    "Could not trash information for user: {}, Error: {}",
                    user_id,
                    e
                ),
            };
        });
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting Bot...");
    Bot::start().await;
}
