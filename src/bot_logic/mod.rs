use std::convert::Infallible;
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
    ratelimit::{limit_all_chats, nonzero, KeyedRateLimitHandler},
    session::{backend::fs::FilesystemBackend, SessionManager},
    types::{
        KeyboardButton, Message,
        MessageData::{Location, Text},
        ParseMode::Markdown,
        ReplyKeyboardMarkup,
    },
    Api, Dispatcher,
};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use tokio::sync::mpsc;

use crate::bot_logic::telegram_tool::send_message;
use crate::location_lookup::{LocationLookup, LocationResult, Lookup};
use crate::trash_dates::{RequestPerformer, Street};

mod menu;
mod strings;

#[derive(Serialize, Deserialize)]
enum States {
    Start,
    MainMenu,
    Search,
    SearchManually,
    SearchManuallyKeyboard,
    SearchManuallyHouseNumber,
    SearchManuallyHouseNumberKeyboard,
    SearchAskIfOk,
    Remove,
}

impl State for States {
    fn new() -> Self {
        States::Start
    }
}

pub struct Bot;

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

async fn add_user(
    request_performer: &RequestPerformer,
    api: Api,
    telegram_chat_id: i64,
    street: Option<i64>,
    house_number: Option<String>,
) {
    use crate::bot_logic::strings::*;

    log::info!("User entered the house correctly, updating user profile.");

    send_message(
        api.clone(),
        SendMessage::new(telegram_chat_id, MESSAGE_SAVE_LOCATION),
    )
    .await;

    match request_performer
        .add_user(telegram_chat_id, street, house_number)
        .await
    {
        Ok(_) => {
            send_message(
                api,
                SendMessage::new(telegram_chat_id, MESSAGE_CONFIRM_ADDRESS_ADDED),
            )
            .await
        }
        Err(_) => {
            send_message(
                api,
                SendMessage::new(telegram_chat_id, MESSAGE_ERROR_ADDRESS_ADDED),
            )
            .await
        }
    };
}

async fn send_affirmative_or_negative(
    api: Api,
    telegram_chat_id: i64,
    switch: bool,
    positive_message: &str,
    negative_message: &str,
) {
    match switch {
        true => send_message(api, SendMessage::new(telegram_chat_id, positive_message)).await,
        false => send_message(api, SendMessage::new(telegram_chat_id, negative_message)).await,
    };
}

mod telegram_tool {
    use backoff::future::retry;
    use backoff::Error::Transient;
    use backoff::ExponentialBackoff;
    use carapax::methods::SendMessage;
    use carapax::Api;

    pub async fn send_message(api: Api, to_send: SendMessage) {
        retry(ExponentialBackoff::default(), || async {
            api.execute(to_send.clone()).await.map_err(Transient)
        })
        .await
        .map_err(|e| {
            log::error!(
                "Error while sending telegram message after multiple retries: {}",
                e
            )
        })
        .unwrap();
    }
}

#[dialogue]
async fn bot_dialogue(
    state: States,
    context: &Context,
    input: Message,
) -> Result<DialogueResult<States>, Infallible> {
    use self::menu::*;
    use self::strings::*;
    use self::States::*;
    use telegram_tool::send_message;

    let chat_id = input.get_chat_id();
    let user = input.get_user().unwrap();
    let first_name = user.first_name.clone();
    let mut session = context.session_manager.get_session(&input).unwrap();
    let api = context.api.clone();

    #[allow(clippy::eval_order_dependence)]
    Ok(match state {
        Start => {
            send_message(
                context.api.clone(),
                SendMessage::new(
                    chat_id,
                    format!(
                        "{}{}{}!\n{}",
                        HELLO,
                        if !first_name.is_empty() { " " } else { "" },
                        first_name,
                        MESSAGE_ASK_WHAT_USER_WANTS
                    ),
                )
                .reply_markup(
                    ReplyKeyboardMarkup::from(vec![
                        vec![
                            KeyboardButton::new(MainMenuQuestion::Search.to_string()),
                            KeyboardButton::new(MainMenuQuestion::ToggleNotifications.to_string()),
                            KeyboardButton::new(
                                MainMenuQuestion::ManualRequestTomorrow.to_string(),
                            ),
                        ],
                        vec![
                            KeyboardButton::new(MainMenuQuestion::Delete.to_string()),
                            KeyboardButton::new(MainMenuQuestion::RequestData.to_string()),
                        ],
                    ])
                    .one_time_keyboard(false)
                    .resize_keyboard(false),
                ),
            )
            .await;

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

                            send_message(
                                api,
                                SendMessage::new(chat_id, MESSAGE_ASK_FOR_MANUAL_ENTRY),
                            )
                            .await;

                            Next(SearchManually)
                        }
                        Ok(location_result) => {
                            session.set("location", &location_result).await.unwrap();

                            match context
                                .request_performer
                                .get_street_id(location_result.street.clone())
                                .await
                            {
                                Ok(street_id) => {
                                    session.set("street_id", &street_id).await.unwrap();
                                    session
                                        .set("street_number", &location_result.house_number)
                                        .await
                                        .unwrap();

                                    send_message(
                                        api,
                                        SendMessage::new(
                                            chat_id,
                                            format!(
                                                "{} *{}*",
                                                CONFIRM_STREET_AND_NUMBER, location_result
                                            ),
                                        )
                                        .reply_markup(
                                            ReplyKeyboardMarkup::from(vec![
                                                vec![KeyboardButton::new(
                                                    LocationQuestion::Correct.to_string(),
                                                )],
                                                vec![KeyboardButton::new(
                                                    LocationQuestion::NumberFalse.to_string(),
                                                )],
                                                vec![KeyboardButton::new(
                                                    LocationQuestion::AllFalse.to_string(),
                                                )],
                                            ])
                                            .one_time_keyboard(true)
                                            .resize_keyboard(true),
                                        )
                                        .parse_mode(Markdown),
                                    )
                                    .await;

                                    Next(SearchAskIfOk)
                                }
                                Err(e) => {
                                    log::error!("{}", e);

                                    send_message(
                                        api,
                                        SendMessage::new(chat_id, MESSAGE_SEARCH_COULD_NOT_FIND),
                                    )
                                    .await;

                                    Next(SearchManually)
                                }
                            }
                        }
                    }
                }
                Text(_) => {
                    send_message(api, SendMessage::new(chat_id, MESSAGE_ENTER_STREET_NAME)).await;
                    Next(SearchManually)
                }
                _ => Next(Start),
            }
        }
        SearchManually => match input.data {
            Text(t) => {
                match context
                    .request_performer
                    .search_similar_streets(t.data)
                    .await
                {
                    Ok(search_results) => {
                        session.set("street_search", &search_results).await.unwrap();

                        send_message(api, {
                            let mut reply_keyboard_rows: Vec<Vec<KeyboardButton>> =
                                Vec::with_capacity(search_results.len());

                            for street in search_results {
                                reply_keyboard_rows.push(vec![KeyboardButton::new(street.street)]);
                            }
                            reply_keyboard_rows
                                .push(vec![KeyboardButton::new(MENU_NO_STREET_CORRECT)]);

                            SendMessage::new(chat_id, MESSAGE_CONFIRM_ONE_OF_THE_STREETS)
                                .reply_markup(
                                    ReplyKeyboardMarkup::from_vec(reply_keyboard_rows)
                                        .resize_keyboard(true)
                                        .one_time_keyboard(true),
                                )
                        })
                        .await;

                        Next(SearchManuallyKeyboard)
                    }
                    Err(e) => {
                        log::error!("Finding streets failed: {}", e);

                        send_message(api, SendMessage::new(chat_id, MESSAGE_ERROR_STREET_SEARCH))
                            .await;

                        Next(Start)
                    }
                }
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
                        send_message(api.clone(), SendMessage::new(chat_id, HOUSE_NUMBER_MESSAGE))
                            .await;

                        was_successful = true;
                        break;
                    }
                }

                if was_successful {
                    Next(SearchManuallyHouseNumber)
                } else {
                    send_message(api, SendMessage::new(chat_id, HELP_MESSAGE)).await;

                    Next(SearchManually)
                }
            }
            _ => Next(Start),
        },
        SearchManuallyHouseNumber => match input.data {
            Text(t) => {
                log::info!("Ask the user whether the house number is correct.");

                send_message(
                    api,
                    SendMessage::new(
                        chat_id,
                        format!(
                            "{}: {}?\n{}",
                            HOUSE_NUMBER_QUESTION_1, t.data, HOUSE_NUMBER_QUESTION_2
                        ),
                    )
                    .reply_markup(
                        ReplyKeyboardMarkup::from_vec(vec![
                            vec![KeyboardButton::new(YES)],
                            vec![KeyboardButton::new(NO)],
                        ])
                        .resize_keyboard(true)
                        .one_time_keyboard(true),
                    ),
                )
                .await;

                session.set("street_number", &t.data).await.unwrap();

                Next(SearchManuallyHouseNumberKeyboard)
            }
            _ => Next(Start),
        },
        SearchManuallyHouseNumberKeyboard => match input.data {
            Text(t) => match &t.data[..] {
                YES => {
                    add_user(
                        &context.request_performer,
                        api,
                        chat_id,
                        session.get("street_id").await.unwrap(),
                        session.get("street_number").await.unwrap(),
                    )
                    .await;

                    Exit
                }
                _ => {
                    log::info!("User entered the house number wrong, trying again.");

                    send_message(api, SendMessage::new(chat_id, HOUSE_NUMBER_MESSAGE)).await;

                    Next(SearchManuallyHouseNumber)
                }
            },
            _ => Exit,
        },
        SearchAskIfOk => match input.data {
            Text(t) => {
                log::info!("Found automatic search answer: {}", t.data);

                match LocationQuestion::from_str(&t.data).unwrap() {
                    LocationQuestion::Correct => {
                        add_user(
                            &context.request_performer,
                            api,
                            chat_id,
                            session.get("street_id").await.unwrap(),
                            session.get("street_number").await.unwrap(),
                        )
                        .await;
                        Exit
                    }
                    LocationQuestion::NumberFalse => {
                        send_message(api, SendMessage::new(chat_id, MESSAGE_ENTER_HOUSE_NUMBER))
                            .await;
                        Next(SearchManuallyHouseNumber)
                    }
                    LocationQuestion::AllFalse => {
                        send_message(api, SendMessage::new(chat_id, MESSAGE_ENTER_STREET_NAME))
                            .await;
                        Next(SearchManually)
                    }
                }
            }
            _ => Next(Start),
        },
        Remove => match input.data {
            Text(t) => match &t.data[..] {
                YES => {
                    let worked = context.request_performer.remove_user_data(chat_id).await;

                    send_affirmative_or_negative(
                        api,
                        chat_id,
                        worked.unwrap_or(false),
                        MESSAGE_DELETED,
                        NO_DELETE_MSG,
                    )
                    .await;
                    Exit
                }
                _ => {
                    send_message(api, SendMessage::new(chat_id, MESSAGE_NOTHING_HAPPENS)).await;
                    Exit
                }
            },
            _ => Exit,
        },
        MainMenu => match input.data {
            Text(t) => match MainMenuQuestion::from_str(&t.data) {
                Ok(main_menu_question) => match main_menu_question {
                    MainMenuQuestion::Search => {
                        log::info!("Starting search dialog.");

                        let row = vec![
                            vec![KeyboardButton::new(MENU_ENTER_MANUALLY)],
                            vec![KeyboardButton::new(MENU_FIND_AUTOMATICALLY).request_location()],
                        ];

                        let markup = ReplyKeyboardMarkup::from(row)
                            .one_time_keyboard(true)
                            .resize_keyboard(true);

                        send_message(
                            api,
                            SendMessage::new(chat_id, MESSAGE_ASK_SEARCH_MODE).reply_markup(markup),
                        )
                        .await;

                        Next(Search)
                    }
                    MainMenuQuestion::ToggleNotifications => {
                        log::info!("Benachrichtigungen");

                        match context
                            .request_performer
                            .get_notification_status(chat_id)
                            .await
                        {
                            Ok(t) => {
                                match context
                                    .request_performer
                                    .set_notification(chat_id, !t)
                                    .await
                                {
                                    Ok(new_state) => {
                                        send_affirmative_or_negative(
                                            api,
                                            chat_id,
                                            new_state,
                                            MESSAGE_NOTIFICATIONS_ACTIVATED,
                                            MESSAGE_NOTIFICATIONS_DEACTIVATED,
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "error while changing notification status: {}",
                                            e
                                        );
                                        send_message(
                                            api,
                                            SendMessage::new(
                                                chat_id,
                                                MESSAGE_ERROR_CHANGE_NOTIFICATION,
                                            ),
                                        )
                                        .await;
                                    }
                                };

                                Next(MainMenu)
                            }
                            Err(e) => {
                                log::error!("{}", e);

                                send_message(
                                    api,
                                    SendMessage::new(chat_id, MESSAGE_CHANGE_NOTIFICATION_NEGATIVE),
                                )
                                .await;

                                Next(MainMenu)
                            }
                        }
                    }
                    MainMenuQuestion::ManualRequestTomorrow => {
                        log::info!("Manual request for tomorrows garbage dates.");

                        match context.request_performer.get_tomorrows_trash(chat_id).await {
                            Ok(t) => {
                                let mut trash: String = t
                                    .into_iter()
                                    .map(|a| a.name)
                                    .collect::<Vec<String>>()
                                    .join(", ");
                                if trash.is_empty() {
                                    trash = String::from(MESSAGE_NO_TRASH_TOMORROW);
                                } else {
                                    trash = String::from(MESSAGE_TRASH_TOMORROW) + trash.as_str();
                                }
                                send_message(api, SendMessage::new(chat_id, trash))
                            }
                            Err(e) => {
                                log::error!("Could not get tomorrows trash dates for manual user request: {}", e);
                                send_message(api, SendMessage::new(chat_id, MESSAGE_ERROR_REQUEST))
                            }
                        }.await;

                        Next(MainMenu)
                    }
                    MainMenuQuestion::Delete => {
                        log::info!("User data deletion: main menu");

                        let row = vec![
                            vec![KeyboardButton::new(YES)],
                            vec![KeyboardButton::new(NO)],
                        ];
                        let markup = ReplyKeyboardMarkup::from(row)
                            .one_time_keyboard(true)
                            .resize_keyboard(true);

                        send_message(
                            api,
                            SendMessage::new(chat_id, DELETION).reply_markup(markup),
                        )
                        .await;

                        Next(Remove)
                    }
                    MainMenuQuestion::RequestData => {
                        log::info!("User data request: main menu");

                        match context.request_performer.get_my_user_data(chat_id).await {
                            Ok(user_data) => {
                                send_message(
                                    api,
                                    SendMessage::new(
                                        chat_id,
                                        user_data
                                            .iter()
                                            .map(|(a, b)| a.to_owned() + ": " + b)
                                            .collect::<Vec<String>>()
                                            .join("\n")
                                            .to_string(),
                                    ),
                                )
                                .await;
                            }
                            Err(e) => {
                                log::error!("failed requesting user data: {}", e);
                                send_message(
                                    api,
                                    SendMessage::new(chat_id, MESSAGE_ERROR_REQUEST_USER_DATA),
                                )
                                .await;
                            }
                        };

                        Next(MainMenu)
                    }
                },
                Err(e) => {
                    log::error!("An error occurred while parsing states: {}", e);
                    Exit
                }
            },
            _ => Exit,
        },
    })
}

fn dates_to_message<T>(something: &[T]) -> String
where
    T: ToString,
{
    if let [] = something {
        Default::default()
    } else {
        something
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(", ")
    }
}

impl Bot {
    pub async fn scheduler() {
        use clokwerk::{AsyncScheduler, Job, TimeUnits};

        let mut scheduler = AsyncScheduler::with_tz(chrono_tz::Europe::Berlin);

        scheduler.every(1.day()).at("16:00:00").run(|| async {
            let request_performer = RequestPerformer::from_env();

            let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
            let api: Api = Api::new(token).unwrap();

            match request_performer.get_active_users_tomorrow().await {
                Ok(users) => {
                    for user in users {
                        send_message(
                            api.clone(),
                            SendMessage::new(user.client_id, dates_to_message(&user.dates[..])),
                        )
                        .await;
                    }
                }
                Err(e) => log::warn!("Error while getting trash dates: {}", e),
            };
        });

        loop {
            scheduler.run_pending().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    pub async fn start() {
        // Start notificator
        log::info!("Start daily notification service...");
        tokio::spawn(async { Bot::scheduler().await });

        let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
        let api: Api = Api::new(token).expect("Failed to create API");
        let (lookup_request_sender, lookup_request_receiver) = mpsc::channel::<Lookup>(32);

        log::info!("Starting geolocation lookup service.");
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

        log::info!("Starting message handling...");
        LongPoll::new(api, dispatcher).run().await;
    }
}
