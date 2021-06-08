use crate::location_lookup::{LocationLookup, LocationResult, Lookup};
use anyhow::Error;
use carapax::{
    dialogue::{
        dialogue, Dialogue,
        DialogueResult::{self, Exit, Next},
        State,
    },
    longpoll::LongPoll,
    methods::SendMessage,
    ratelimit::{
        limit_all_chats, limit_all_users, nonzero, DirectRateLimitHandler, KeyedRateLimitHandler,
        RateLimitList,
    },
    session::{backend::fs::FilesystemBackend, SessionManager},
    types::{
        KeyboardButton, Message,
        MessageData::{Location, Text},
        ParseMode::Markdown,
        ReplyKeyboardMarkup, ReplyMarkup,
    },
    Api, Dispatcher,
};
use graphql_client::PathFragment::Key;
use serde::{Deserialize, Serialize};
use serde_json::de::Read;
use std::convert::Infallible;
use std::env;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize)]
enum States {
    Start,
    MainMenu,
    Search,
    SearchManually,
    SearchAskIfOk,
    Add,
    Remove,
    ToggleNotifications,
}

mod question_helpers {
    use std::convert::TryFrom;
    use std::fmt::Formatter;
    use std::str::FromStr;

    pub enum LocationQuestion {
        Correct,
        NumberFalse,
        AllFalse,
    }

    static CORRECT: &str = "Ja, beides stimmt!";
    static NUMBER_FALSE: &str = "Nein, die Hausnummer stimmt nicht!";
    static ALL_FALSE: &str = "Nein, beides ist falsch!";

    impl std::fmt::Display for LocationQuestion {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                self::LocationQuestion::Correct => write!(f, "Ja, beides stimmt!"),
                self::LocationQuestion::NumberFalse => {
                    write!(f, "Nein, die Hausnummer stimmt nicht!")
                }
                self::LocationQuestion::AllFalse => write!(f, "Nein, beides ist falsch!"),
            }
        }
    }

    pub enum MainMenuQuestion {
        ToggleNotifications,
        Search,
    }

    static SEARCH: &str = "Straße auswählen/ändern";
    static NOTIFICATION: &str = "Benachrichtigungen ein-/ausschalten";

    impl std::fmt::Display for MainMenuQuestion {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                self::MainMenuQuestion::ToggleNotifications => {
                    write!(f, NOTIFICATION)
                }
                self::MainMenuQuestion::Search => {
                    write!(f, SEARCH)
                }
            }
        }
    }

    impl FromStr for MainMenuQuestion {
        type Err = &'static str;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                SEARCH => Ok(MainMenuQuestion::Search),
                NOTIFICATION => Ok(MainMenuQuestion::ToggleNotifications),
                _ => Err("Could not convert to MainMenuQuestion."),
            }
        }
    }
}

impl State for States {
    fn new() -> Self {
        States::Start
    }
}

pub struct Bot {}

struct Context {
    api: Api,
    session_manager: SessionManager<FilesystemBackend>,
    sender: mpsc::Sender<Lookup>,
}

async fn get_reverse_location(
    location: &carapax::types::Location,
    sender: &mpsc::Sender<Lookup>,
) -> Result<LocationResult, Error> {
    let (location_result_sender, location_result_answer) =
        tokio::sync::oneshot::channel::<Result<Option<LocationResult>, Error>>();

    sender
        .send(Lookup {
            longitude: location.longitude,
            latitude: location.latitude,
            responder: location_result_sender,
        })
        .await?;

    Ok(location_result_answer.await??.unwrap())
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
    let last_name = match user.last_name.as_ref() {
        Some(t) => &t[..],
        None => "",
    };
    let mut session = context.session_manager.get_session(&input).unwrap();

    Ok(match state {
        Start => {
            context
                .api
                .execute(SendMessage::new(chat_id, format!("Hallo {}!", first_name)))
                .await
                .unwrap();

            context
                .api
                .execute(
                    SendMessage::new(chat_id, "Was möchtest du tun?").reply_markup(
                        ReplyKeyboardMarkup::from(vec![
                            vec![KeyboardButton::new(
                                question_helpers::MainMenuQuestion::ToggleNotifications.to_string(),
                            )],
                            vec![KeyboardButton::new(
                                question_helpers::MainMenuQuestion::Search.to_string(),
                            )],
                        ])
                        .one_time_keyboard(true),
                    ),
                )
                .await
                .unwrap();

            Next(MainMenu)
        }
        Search => {
            log::info!("Handling search dialog.");
            match input.data {
                Location(location) => {
                    log::info!("Found location, ask the user if it's correct.");

                    let location_result = get_reverse_location(&location, &context.sender).await;
                    if let Err(e) = location_result {
                        log::warn!("Could not find reverse location: {}", e);
                        context.api.execute(SendMessage::new(chat_id, "Konte deinen Standort nicht zuordnen, bitte gib deine Addresse manuel ein.")).await.unwrap();
                        return Ok(Next(SearchManually));
                    }
                    let location_result = location_result.unwrap();
                    session.set("location", &location_result).await.unwrap();
                    context
                        .api
                        .execute(
                            SendMessage::new(
                                chat_id,
                                format!(
                                    "Ist das die korrekte Straße und Hausnummer? *{}*",
                                    location_result
                                ),
                            )
                            .reply_markup(
                                ReplyKeyboardMarkup::from(vec![
                                    vec![KeyboardButton::new(
                                        question_helpers::LocationQuestion::Correct.to_string(),
                                    )],
                                    vec![KeyboardButton::new(
                                        question_helpers::LocationQuestion::NumberFalse.to_string(),
                                    )],
                                    vec![KeyboardButton::new(
                                        question_helpers::LocationQuestion::AllFalse.to_string(),
                                    )],
                                ])
                                .one_time_keyboard(true)
                                .resize_keyboard(true),
                            )
                            .parse_mode(Markdown),
                        )
                        .await
                        .unwrap();
                    Next(SearchAskIfOk)
                }
                Text(t) => Next(SearchManually),
                _ => Next(Start),
            }
        }
        SearchManually => Next(Start),
        SearchAskIfOk => Next(Start),
        Add => Next(Start),
        Remove => Next(Start),
        ToggleNotifications => Next(Start),
        MainMenu => match input.data {
            Text(t) => match question_helpers::MainMenuQuestion::from(t.data) {
                question_helpers::MainMenuQuestion::Search => {
                    log::info!("Starting search dialog.");

                    let row = vec![
                        vec![KeyboardButton::new("Selbst eingeben")],
                        vec![KeyboardButton::new("Automatisch finden").request_location()],
                    ];

                    let markup = ReplyKeyboardMarkup::from(row)
                        .one_time_keyboard(false)
                        .resize_keyboard(false);
                    let reply_markup = ReplyMarkup::from(markup);

                    context.api.execute(SendMessage::new(chat_id, "Willst du deine Addresse selbst eingeben oder willst du sie automatisch finden lassen?").reply_markup(reply_markup)).await.unwrap();

                    Next(Search)
                }
                question_helpers::MainMenuQuestion::ToggleNotifications => {
                    log::info!("Benachrichtigungen");
                    Next(ToggleNotifications)
                }
            },
            _ => Next(Start),
        },
    })
}

impl Bot {
    pub async fn start() {
        let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
        let api: Api = Api::new(token).expect("Failed to create API");
        let (lookup_request_sender, lookup_request_receiver) = mpsc::channel::<Lookup>(32);

        let mut lookup_device = LocationLookup::new(lookup_request_receiver).await;

        tokio::spawn(async move {
            lookup_device.start().await;
        });

        let tmpdir = tempdir().expect("Failed to create temp directory");
        let dialogue_name = "BasicDialogue"; // unique dialogue name used to store state
        let session_manager = SessionManager::new(FilesystemBackend::new(tmpdir.path()));

        let mut dispatcher = Dispatcher::new(Context {
            session_manager: session_manager.clone(),
            api: api.clone(),
            sender: lookup_request_sender.clone(),
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
