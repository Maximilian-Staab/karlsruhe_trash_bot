use std::env;
use std::fmt::Formatter;

use anyhow::Error;
use chrono::{NaiveDate, Utc};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

static HASURA_HEADER: &str = "x-hasura-admin-secret";

#[derive(Debug)]
pub struct InvalidDateError;

type Date = chrono::NaiveDate;
type Timestamptz = chrono::DateTime<Utc>;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TrashType {
    Organic,
    Recycling,
    Paper,
    Miscellaneous,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct User {
    first_name: String,
    last_name: String,
    pub client_id: i64,
    pub dates: Vec<TrashDate>,
}

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/tomorrow_for_user.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct TomorrowForUser;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/search_street.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct SearchStreet;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/tomorrow_for_all.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct TomorrowForAll;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/users.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct ActiveUsers;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/add_user.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct AddUser;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/set_notification.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct SetNotification;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/delete_user.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct DeleteUser;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/notification_status.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct NotificationStatus;

#[derive(Debug, Clone)]
pub struct RequestPerformer {
    secret: String,
    endpoint: String,
    client: Client,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TrashDate {
    pub date: NaiveDate,
    pub trash_type: TrashType,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Street {
    pub street: String,
    pub id: i64,
}

fn word_to_uppercase(string: &str) -> String {
    let mut c = string.chars();
    match c.next() {
        None => Default::default(),
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
}

fn sentence_to_uppercase(s: &str) -> String {
    s.split_whitespace()
        .map(word_to_uppercase)
        .collect::<Vec<String>>()
        .join(" ")
}

impl From<search_street::SearchStreetSearchStreets> for Street {
    fn from(ss: search_street::SearchStreetSearchStreets) -> Self {
        Street {
            street: sentence_to_uppercase(&ss.name),
            id: ss.id,
        }
    }
}

impl From<active_users::ActiveUsersUsers> for User {
    fn from(au: active_users::ActiveUsersUsers) -> Self {
        User {
            client_id: au.telegram_chat_id,
            first_name: au.first_name.unwrap_or_default(),
            last_name: au.last_name.unwrap_or_default(),
            dates: Vec::new(),
        }
    }
}

impl From<tomorrow_for_all::TomorrowForAllUsers> for User {
    fn from(au: tomorrow_for_all::TomorrowForAllUsers) -> Self {
        User {
            client_id: au.telegram_chat_id,
            first_name: Default::default(),
            last_name: Default::default(),
            dates: au.dates.into_iter().map(TrashDate::from).collect(),
        }
    }
}

impl From<tomorrow_for_all::TomorrowForAllUsersDates> for TrashDate {
    fn from(tat: tomorrow_for_all::TomorrowForAllUsersDates) -> Self {
        TrashDate {
            name: String::from(&tat.trash_type_by_trash_type.name[..]),
            date: tat.date,
            trash_type: TrashType::from(&tat.trash_type_by_trash_type.name[..]),
        }
    }
}

impl From<&str> for TrashType {
    fn from(string: &str) -> Self {
        match string {
            "Bioabfall" => TrashType::Organic,
            "Wertstoff" => TrashType::Recycling,
            "Papier" => TrashType::Paper,
            "Restmüll" => TrashType::Miscellaneous,
            _ => panic!("Could not find the selected type of trash: {}", string),
        }
    }
}

impl From<tomorrow_for_user::TomorrowForUserDates> for TrashDate {
    fn from(tat: tomorrow_for_user::TomorrowForUserDates) -> Self {
        TrashDate {
            name: String::from(&tat.trash_type_by_trash_type.name[..]),
            date: tat.date,
            trash_type: TrashType::from(&tat.trash_type_by_trash_type.name[..]),
        }
    }
}

impl std::fmt::Display for TrashDate {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.date)
    }
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {:?}",
            self.first_name, self.last_name, self.client_id
        )
    }
}

impl RequestPerformer {
    pub fn new(secret: String, endpoint: String) -> Self {
        RequestPerformer {
            secret,
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            env::var("HASURA_SECRET")
                .expect("Hasura header authorisation not set, set env variable 'HASURA_SECRET'."),
            env::var("HASURA_ENDPOINT")
                .expect("Hasura endpoint url is missing, set env variable 'HASURA_ENDPOINT'"),
        )
    }

    pub async fn get_tomorrows_trash(&self, user_id: i64) -> Result<Vec<TrashDate>, Error> {
        // let variables: trash_at_date::Variables();
        let request_body = TomorrowForUser::build_query(tomorrow_for_user::Variables { user_id });
        let response_data: tomorrow_for_user::ResponseData = self.send_request(&request_body).await;

        Ok(response_data
            .dates
            .into_iter()
            .map(TrashDate::from)
            .collect())
    }

    fn log_errors<T>(&self, response: Response<T>) -> Option<T> {
        if let Some(errors) = &response.errors {
            log::error!("Something failed!");

            for error in errors {
                log::error!("{:?}", error);
            }
        }
        response.data
    }

    pub async fn get_street_id(&self, street_name: String) -> Option<i64> {
        let response_body = SearchStreet::build_query(search_street::Variables {
            limit: Some(1i64),
            name: Some(street_name),
        });
        let result: search_street::ResponseData = self.send_request(&response_body).await;
        Some(result.search_streets.into_iter().next().unwrap().id)
    }

    pub async fn get_notification_status(&self, telegram_chat_id: i64) -> Option<bool> {
        let response_body = NotificationStatus::build_query(notification_status::Variables {
            user_id: telegram_chat_id,
        });

        let result: notification_status::ResponseData = self.send_request(&response_body).await;
        Some(result.users_by_pk?.enabled_notifications)
    }

    pub async fn search_similar_streets(&self, street_name: String) -> Option<Vec<Street>> {
        let response_body = SearchStreet::build_query(search_street::Variables {
            limit: Some(5i64),
            name: Some(street_name),
        });
        let result: search_street::ResponseData = self.send_request(&response_body).await;
        Some(
            result
                .search_streets
                .into_iter()
                .map(Street::from)
                .collect(),
        )
    }

    pub async fn remove_user_data(&self, telegram_chat_id: i64) -> Option<bool> {
        let response_body = DeleteUser::build_query(delete_user::Variables {
            telegram_chat_id: Some(telegram_chat_id),
        });
        let result = self.send_request::<graphql_client::QueryBody<delete_user::Variables>, delete_user::ResponseData>(&response_body).await;
        Some(result.delete_users?.affected_rows == 1)
    }

    pub async fn add_user(
        &self,
        first_name: Option<String>,
        last_name: Option<String>,
        telegram_chat_id: i64,
        street: i64,
        house_number: Option<String>,
    ) {
        let response_body = AddUser::build_query(add_user::Variables {
            last_name,
            first_name,
            telegram_chat_id: Some(telegram_chat_id),
            street: Some(street),
            house_number,
        });

        self.send_request::<graphql_client::QueryBody<add_user::Variables>, add_user::ResponseData>(&response_body).await;
    }

    async fn send_request<T: Serialize + ?Sized, R: DeserializeOwned>(&self, json: &T) -> R {
        self.log_errors(
            self.client
                .post(&self.endpoint)
                .header(HASURA_HEADER, &self.secret)
                .json(json)
                .send()
                .await
                .unwrap()
                .json::<graphql_client::Response<R>>()
                .await
                .unwrap(),
        )
        .expect("No response Data")
    }

    pub async fn set_notification(&self, telegram_chat_id: i64, notifications: bool) {
        let response_body = SetNotification::build_query(set_notification::Variables {
            telegram_chat_id,
            enabled_notifications: notifications,
        });

        self.send_request::<graphql_client::QueryBody<set_notification::Variables>, set_notification::ResponseData>(
            &response_body,
        ).await;
    }

    pub async fn get_active_users(&self) -> Result<Vec<User>, Error> {
        let response_body = ActiveUsers::build_query(active_users::Variables {});
        let response_data: active_users::ResponseData = self.send_request(&response_body).await;

        Ok(response_data.users.into_iter().map(User::from).collect())
    }

    pub async fn get_active_users_tomorrow(&self) -> Result<Vec<User>, Error> {
        // let variables: trash_at_date::Variables();
        let request_body = TomorrowForAll::build_query(tomorrow_for_all::Variables {});
        let response_data: tomorrow_for_all::ResponseData = self.send_request(&request_body).await;

        Ok(response_data.users.into_iter().map(User::from).collect())
    }
}
