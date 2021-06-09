use crate::bot_logic::question_helpers::LocationQuestion;
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
use std::convert::{Infallible, TryInto};
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc;
use trash_bot::trash_dates::{RequestPerformer, Street};

#[derive(Serialize, Deserialize)]
enum States {
    Start,
    MainMenu,
    Search,
    SearchManually,
    SearchManuallyHouseNumber,
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

    const CORRECT: &str = "Ja, beides stimmt!";
    const NUMBER_FALSE: &str = "Nein, die Hausnummer stimmt nicht!";
    const ALL_FALSE: &str = "Nein, beides ist falsch!";

    impl std::fmt::Display for LocationQuestion {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                self::LocationQuestion::Correct => write!(f, "{}", CORRECT),
                self::LocationQuestion::NumberFalse => {
                    write!(f, "{}", NUMBER_FALSE)
                }
                self::LocationQuestion::AllFalse => write!(f, "{}", ALL_FALSE),
            }
        }
    }

    impl FromStr for LocationQuestion {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                CORRECT => Ok(Self::Correct),
                NUMBER_FALSE => Ok(Self::NumberFalse),
                ALL_FALSE => Ok(Self::AllFalse),
                _ => Err(format!("Could not convert to LocationQuestion: {}", s)),
            }
        }
    }

    pub enum MainMenuQuestion {
        ToggleNotifications,
        Search,
    }

    const SEARCH: &str = "Straße auswählen/ändern";
    const NOTIFICATION: &str = "Benachrichtigungen ein-/ausschalten";

    impl std::fmt::Display for MainMenuQuestion {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                self::MainMenuQuestion::ToggleNotifications => {
                    write!(f, "{}", NOTIFICATION)
                }
                self::MainMenuQuestion::Search => {
                    write!(f, "{}", SEARCH)
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
    request_performer: RequestPerformer,
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
    let first_name = Some(user.first_name.clone());
    let last_name = user.last_name.clone();
    let mut session = context.session_manager.get_session(&input).unwrap();

    Ok(match state {
        Start => {
            context
                .api
                .execute(
                    SendMessage::new(
                        chat_id,
                        format!(
                            "Hallo{}{}!\nWas möchtest du tun?",
                            if first_name.is_some() { " " } else { "" },
                            first_name.as_deref().unwrap_or("")
                        ),
                    )
                    .reply_markup(
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
                Text(_) => {
                    context
                        .api
                        .execute(SendMessage::new(
                            chat_id,
                            "Gib den Namen deiner Straße ein, um Vorschläge zu erhalten.",
                        ))
                        .await
                        .unwrap();
                    Next(SearchManually)
                }
                _ => Next(Start),
            }
        }
        SearchManually => {
            if let Text(street_request) = input.data {
                let search_results = context
                    .request_performer
                    .search_similar_streets(street_request.data)
                    .await
                    .unwrap();

                session.set("street_search", &search_results);

                let mut reply_keyboard_rows: Vec<Vec<KeyboardButton>> =
                    Vec::with_capacity(search_results.len());

                for street in search_results {
                    reply_keyboard_rows.push(vec![KeyboardButton::new(street.street)]);
                }
            }
            Next(Start)
        }
        SearchManuallyHouseNumber => Next(Start),
        SearchAskIfOk => {
            match input.data {
                Text(t) => {
                    match question_helpers::LocationQuestion::from_str(&t.data).unwrap() {
                        LocationQuestion::Correct => {
                            context
                        .api
                        .execute(SendMessage::new(
                            chat_id,
                            "Speichere deinen Standort fuer die Abfrage der Müllabholdaten.",
                        ))
                        .await
                        .unwrap();

                            let location: LocationResult =
                                session.get("location").await.unwrap().unwrap();
                            let has_street_id = context
                                .request_performer
                                .get_street_id(location.street)
                                .await;

                            match has_street_id {
                                Some(street_id) => {
                                    context
                                        .request_performer
                                        .add_user(
                                            first_name,
                                            last_name,
                                            chat_id,
                                            street_id,
                                            location.house_number,
                                        )
                                        .await;
                                    context.api.execute(SendMessage::new(
                                        chat_id,
                                        "Addresse hinzugefügt!",
                                    ));

                                    Next(Start)
                                }
                                None => {
                                    context.api.execute(SendMessage::new(chat_id, "Konnte deine Straße nicht in der Datenbank finden. Bitte gib den Namen deiner Straße ein um Vorschläge anzuzeigen:",)).await.unwrap();

                                    Next(SearchManually)
                                }
                            }
                        }
                        LocationQuestion::NumberFalse => {
                            context
                                .api
                                .execute(SendMessage::new(
                                    chat_id,
                                    "Bitte gib die Hausnummer an, die du verwenden willst:",
                                ))
                                .await
                                .unwrap();

                            Next(SearchManuallyHouseNumber)
                        }
                        LocationQuestion::AllFalse => {
                            context.api.execute(SendMessage::new(chat_id, "Bitte gib den Namen deiner Straße ein um Vorschläge anzuzeigen:")).await.unwrap();

                            Next(SearchManually)
                        }
                    }
                }
                _ => Next(Start),
            }
        }
        Add => Next(Start),
        Remove => Next(Start),
        ToggleNotifications => Next(Start),
        MainMenu => match input.data {
            Text(t) => match question_helpers::MainMenuQuestion::from_str(&t.data).unwrap() {
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
            request_performer: RequestPerformer::from_env(),
        });

        let (capacity, interval) = (nonzero!(1u32), Duration::from_secs(5));

        dispatcher.add_handler(KeyedRateLimitHandler::new(
            limit_all_chats,
            true,
            capacity,
            interval,
        ));

        dispatcher.add_handler(Dialogue::new(session_manager, dialogue_name, bot_dialogue));

        LongPoll::new(api, dispatcher).run().await;
    }
}
