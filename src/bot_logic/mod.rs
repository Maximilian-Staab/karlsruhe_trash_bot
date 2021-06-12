use std::convert::{Infallible, TryInto};
use std::env;
use std::str::FromStr;
use std::time::Duration;

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
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use tokio::sync::mpsc;

use trash_bot::trash_dates::{RequestPerformer, Street};

use crate::bot_logic::question_helpers::LocationQuestion;
use crate::location_lookup::{LocationLookup, LocationResult, Lookup};

#[derive(Serialize, Deserialize)]
enum States {
    Start,                             // Done
    MainMenu,                          // Done
    Search,                            // Done
    SearchManually,                    // Done
    SearchManuallyKeyboard,            // Done
    SearchManuallyHouseNumber,         // Done
    SearchManuallyHouseNumberKeyboard, // Done
    SearchAskIfOk,                     // Done
    Remove,                            // Done
}

mod question_helpers {
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
        Search,
        ToggleNotifications,
        Delete,
    }

    const SEARCH: &str = "Straße auswählen/ändern";
    const NOTIFICATION: &str = "Benachrichtigungen ein-/ausschalten";
    const DELETE: &str = "Alle Daten löschen";

    impl std::fmt::Display for MainMenuQuestion {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                self::MainMenuQuestion::Search => {
                    write!(f, "{}", SEARCH)
                }
                self::MainMenuQuestion::ToggleNotifications => {
                    write!(f, "{}", NOTIFICATION)
                }
                self::MainMenuQuestion::Delete => {
                    write!(f, "{}", DELETE)
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
                DELETE => Ok(MainMenuQuestion::Delete),
                _ => Err("Could not convert to MainMenuQuestion."),
            }
        }
    }
}

mod messages {
    pub const HOUSE_NUMBER_MESSAGE: &str =
        "Bitte gib deine Hausnummer an (die Entsorgungstermine sind abhängig von der Hausnummer).";

    pub const HELP_MESSAGE: &str = "Versuche den vollständigen Namen deiner Straße anzugeben. Ansonsten stelle sicher, dass deine Straße im Abfuhrkallender von Karlsruhe aufgeführt ist.\n\nGib deine Straße ein:";

    pub const HOUSE_NUMBER_QUESTION_1: &str = "Ist das deine Hausnummer";
    pub const HOUSE_NUMBER_QUESTION_2: &str = "Stelle sicher, dass die Nummer korrekt ist, da sonst möglicherweise keine Entsorgungstermine gefunden werden können.";

    pub const SEARCH_COULD_NOT_FIND: &str = "Konnte deine Straße nicht in der Datenbank finden. Bitte gib den Namen deiner Straße ein um Vorschläge anzuzeigen:";

    pub const DELETION: &str = "Willst du all deine Daten löschen?";
    pub const NO_DELETE_MSG: &str =
        "Konnte deine Daten nicht finden, hast du deine Daten schon gelöscht?";
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
    use messages::*;

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
                                question_helpers::MainMenuQuestion::Search.to_string(),
                            )],
                            vec![KeyboardButton::new(
                                question_helpers::MainMenuQuestion::ToggleNotifications.to_string(),
                            )],
                            vec![KeyboardButton::new(
                                question_helpers::MainMenuQuestion::Delete.to_string(),
                            )],
                        ])
                        .one_time_keyboard(false)
                        .resize_keyboard(true),
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

                    match get_reverse_location(&location, &context.sender).await {
                        Err(e) => {
                            log::warn!("Could not find reverse location: {}", e);
                            context.api.execute(SendMessage::new(chat_id, "Konte deinen Standort nicht zuordnen, bitte gib deine Addresse manuel ein.")).await.unwrap();
                            Next(SearchManually)
                        }
                        Ok(location_result) => {
                            session.set("location", &location_result).await.unwrap();

                            match context
                                .request_performer
                                .get_street_id(location_result.street.clone())
                                .await
                            {
                                Some(street_id) => {
                                    session.set("street_id", &street_id).await.unwrap();
                                    session
                                        .set("street_number", &location_result.house_number)
                                        .await
                                        .unwrap();

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
                                                            question_helpers::LocationQuestion::Correct
                                                                .to_string(),
                                                        )],
                                                        vec![KeyboardButton::new(
                                                            question_helpers::LocationQuestion::NumberFalse
                                                                .to_string(),
                                                        )],
                                                        vec![KeyboardButton::new(
                                                            question_helpers::LocationQuestion::AllFalse
                                                                .to_string(),
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
                                None => {
                                    context
                                        .api
                                        .execute(SendMessage::new(chat_id, SEARCH_COULD_NOT_FIND))
                                        .await
                                        .unwrap();

                                    Next(SearchManually)
                                }
                            }
                        }
                    }
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
        SearchManually => match input.data {
            Text(t) => {
                let search_results = context
                    .request_performer
                    .search_similar_streets(t.data)
                    .await
                    .unwrap();

                session.set("street_search", &search_results).await.unwrap();

                let mut reply_keyboard_rows: Vec<Vec<KeyboardButton>> =
                    Vec::with_capacity(search_results.len());

                for street in search_results {
                    reply_keyboard_rows.push(vec![KeyboardButton::new(street.street)]);
                }
                reply_keyboard_rows.push(vec![KeyboardButton::new("Keine der Straße ist richtig")]);

                context
                    .api
                    .execute(
                        SendMessage::new(chat_id, "Ist deine Straße hier aufgefürt?").reply_markup(
                            ReplyKeyboardMarkup::from_vec(reply_keyboard_rows)
                                .resize_keyboard(true)
                                .one_time_keyboard(true),
                        ),
                    )
                    .await
                    .unwrap();

                Next(SearchManuallyKeyboard)
            }
            _ => Next(Start),
        },
        SearchManuallyKeyboard => match input.data {
            Text(t) => {
                let mut was_successful = false;

                for street in session
                    .get::<&str, Vec<Street>>("street_search")
                    .await
                    .unwrap()
                    .unwrap()
                {
                    if street.street == t.data {
                        session.set("street_id", &street.id).await.unwrap();
                        context
                            .api
                            .execute(SendMessage::new(chat_id, HOUSE_NUMBER_MESSAGE))
                            .await
                            .unwrap();

                        was_successful = true;
                        break;
                    }
                }
                if was_successful {
                    Next(SearchManuallyHouseNumber)
                } else {
                    context
                        .api
                        .execute(SendMessage::new(chat_id, HELP_MESSAGE))
                        .await
                        .unwrap();

                    Next(SearchManually)
                }
            }
            _ => Next(Start),
        },
        SearchManuallyHouseNumber => match input.data {
            Text(t) => {
                log::info!("Ask the user whether the house number is correct.");

                context
                    .api
                    .execute(
                        SendMessage::new(
                            chat_id,
                            format!(
                                "{}: {}?\n{}",
                                HOUSE_NUMBER_QUESTION_1, t.data, HOUSE_NUMBER_QUESTION_2
                            ),
                        )
                        .reply_markup(
                            ReplyKeyboardMarkup::from_vec(vec![
                                vec![KeyboardButton::new("Ja")],
                                vec![KeyboardButton::new("Nein")],
                            ])
                            .resize_keyboard(true)
                            .one_time_keyboard(true),
                        ),
                    )
                    .await
                    .unwrap();

                session.set("street_number", &t.data).await.unwrap();

                Next(SearchManuallyHouseNumberKeyboard)
            }
            _ => Next(Start),
        },
        SearchManuallyHouseNumberKeyboard => match input.data {
            Text(t) => match &t.data[..] {
                "Ja" => {
                    log::info!("User entered the house correctly, updating user profile.");

                    context
                        .request_performer
                        .add_user(
                            first_name,
                            last_name,
                            chat_id,
                            session.get("street_id").await.unwrap().unwrap(),
                            session.get("street_number").await.unwrap(),
                        )
                        .await;

                    context
                        .api
                        .execute(SendMessage::new(chat_id, "Addresse hinzugefügt!"))
                        .await
                        .unwrap();

                    Next(Start)
                }
                _ => {
                    log::info!("User entered the house number wrong, trying again.");
                    context
                        .api
                        .execute(SendMessage::new(chat_id, HOUSE_NUMBER_MESSAGE))
                        .await
                        .unwrap();

                    Next(SearchManuallyHouseNumber)
                }
            },
            _ => Next(Start),
        },
        SearchAskIfOk => {
            match input.data {
                Text(t) => {
                    log::info!("Found automatic search answer: {}", t.data);
                    match question_helpers::LocationQuestion::from_str(&t.data).unwrap() {
                        LocationQuestion::Correct => {
                            context
                                .api
                                .execute(SendMessage::new(
                                    chat_id,
                                    "Speichere deinen Standort für die Abfrage der Entsorgungstermine.",
                                ))
                                .await
                                .unwrap();

                            context
                                .request_performer
                                .add_user(
                                    first_name,
                                    last_name,
                                    chat_id,
                                    session.get("street_id").await.unwrap().unwrap(),
                                    session.get("street_number").await.unwrap().unwrap(),
                                )
                                .await;

                            context
                                .api
                                .execute(SendMessage::new(chat_id, "Addresse hinzugefügt!"))
                                .await
                                .unwrap();

                            Next(Start)
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
                            context.api.execute(SendMessage::new(chat_id, "Bitte gib den Namen deiner Straße ein, um Vorschläge anzuzeigen:")).await.unwrap();

                            Next(SearchManually)
                        }
                    }
                }
                _ => Next(Start),
            }
        }
        Remove => match input.data {
            Text(t) => match &t.data[..] {
                "Ja" => {
                    let worked = context.request_performer.remove_user_data(chat_id).await;

                    match worked.unwrap_or(false) {
                        true => context
                            .api
                            .execute(SendMessage::new(chat_id, "Gelöscht!"))
                            .await
                            .unwrap(),
                        false => context
                            .api
                            .execute(SendMessage::new(chat_id, NO_DELETE_MSG))
                            .await
                            .unwrap(),
                    };

                    Next(Start)
                }
                _ => {
                    context
                        .api
                        .execute(SendMessage::new(chat_id, "Ok, nichts passiert!"))
                        .await
                        .unwrap();
                    Next(Start)
                }
            },
            _ => Next(Start),
        },
        MainMenu => match input.data {
            Text(t) => match question_helpers::MainMenuQuestion::from_str(&t.data) {
                Ok(main_menu_question) => match main_menu_question {
                    question_helpers::MainMenuQuestion::Search => {
                        log::info!("Starting search dialog.");

                        let row = vec![
                            vec![KeyboardButton::new("Selbst eingeben")],
                            vec![KeyboardButton::new("Automatisch finden").request_location()],
                        ];

                        let markup = ReplyKeyboardMarkup::from(row)
                            .one_time_keyboard(true)
                            .resize_keyboard(true);

                        context.api.execute(SendMessage::new(chat_id, "Willst du deine Addresse selbst eingeben oder willst du sie automatisch finden lassen?")
                            .reply_markup(markup)).await.unwrap();

                        Next(Search)
                    }
                    question_helpers::MainMenuQuestion::ToggleNotifications => {
                        log::info!("Benachrichtigungen");

                        match context
                            .request_performer
                            .get_notification_status(chat_id)
                            .await
                        {
                            Some(t) => {
                                context
                                    .request_performer
                                    .set_notification(chat_id, !t)
                                    .await;

                                match !t {
                                    true => context
                                        .api
                                        .execute(SendMessage::new(
                                            chat_id,
                                            "Benachrichtigungen aktiviert",
                                        ))
                                        .await
                                        .unwrap(),
                                    false => context
                                        .api
                                        .execute(SendMessage::new(
                                            chat_id,
                                            "Benachrichtigungen deaktiviert",
                                        ))
                                        .await
                                        .unwrap(),
                                };
                                Next(MainMenu)
                            }
                            None => {
                                context.api.execute(SendMessage::new(chat_id, "Konnte Benachrichtigungsstatus nicht finden, hast du deine Strasse und Hausnummer schon hinzugefügt?")).await.unwrap();
                                Next(MainMenu)
                            }
                        }
                    }
                    question_helpers::MainMenuQuestion::Delete => {
                        log::info!("User data deletion: main menu");

                        let row = vec![
                            vec![KeyboardButton::new("Ja")],
                            vec![KeyboardButton::new("Nein")],
                        ];
                        let markup = ReplyKeyboardMarkup::from(row)
                            .one_time_keyboard(true)
                            .resize_keyboard(true);
                        context
                            .api
                            .execute(SendMessage::new(chat_id, DELETION).reply_markup(markup))
                            .await
                            .unwrap();

                        Next(Remove)
                    }
                },
                Err(e) => {
                    log::error!("An error occurred while parsing states: {}", e);
                    Next(Start)
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

        let (capacity, interval) = (nonzero!(3u32), Duration::from_secs(3));

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
