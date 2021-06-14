mod bot_logic;
mod location_lookup;
pub mod trash_dates;
use crate::bot_logic::Bot;
use log::info;

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting Bot...");
    Bot::start().await;
}
