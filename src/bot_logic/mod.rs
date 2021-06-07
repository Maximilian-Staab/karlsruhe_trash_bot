use carapax::longpoll::LongPoll;
use carapax::methods::SendMessage;
use carapax::{
    dialogue::{
        dialogue, Dialogue,
        DialogueResult::{self, *},
        State,
    },
    ratelimit::{
        limit_all_chats, limit_all_users, nonzero, DirectRateLimitHandler, KeyedRateLimitHandler,
        RateLimitList,
    },
    session::{backend::fs::FilesystemBackend, SessionManager},
    types::Message,
    Api, Dispatcher,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use std::time::Duration;
use tempfile::tempdir;

#[derive(Serialize, Deserialize)]
enum States {
    Start,
    Search,
    Add,
    Remove,
}

impl State for States {
    fn new() -> Self {
        States::Start
    }
}

struct Bot {}

struct Context {
    api: Api,
    session_manager: SessionManager<FilesystemBackend>,
}

#[dialogue]
async fn bot_dialogue(
    state: States,
    context: &Context,
    input: Message,
) -> Result<DialogueResult<States>, Infallible> {
    use self::States::*;

    let chat_id = input.get_chat_id();
    let user = input.get_user().unwrap();
    let first_name = &user.first_name[..];
    let first_name = &user.last_name.unwrap_or_else(|| "".to_string())[..];
    let mut session = context.session_manager.get_session(&input).unwrap();

    Ok(match state {
        Start => {
            context.api.execute(SendMessage);
            Next(Search)
        }
        Search => Next(Add),
        Add => Next(Start),
        Remove => Next(Start),
    })
}

impl Bot {
    pub async fn start() {
        let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
        let api: Api = Api::new(token).expect("Failed to create API");

        let tmpdir = tempdir().expect("Failed to create temp directory");
        let dialogue_name = "BasicDialogue"; // unique dialogue name used to store state
        let session_manager = SessionManager::new(FilesystemBackend::new(tmpdir.path()));

        let mut dispatcher = Dispatcher::new(Context {
            session_manager: session_manager.clone(),
            api: api.clone(),
        });

        let (capacity, interval) = (nonzero!(1u32), Duration::from_secs(5));

        dispatcher.add_handler(KeyedRateLimitHandler::new(
            limit_all_chats,
            true,
            capacity,
            interval,
        ));

        dispatcher.add_handler(Dialogue::new(session_manager, dialogue_name, bot_dialogue));

        let thing = LongPoll::new(api, dispatcher).run().await;
    }
}
