use std::env;
use std::fmt::{Debug, Formatter};

use anyhow::{Error, Result};
use chrono::{NaiveDate, Utc};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    query_path = "graphql/all_user_data.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct UserData;

impl From<user_data::UserDataUsersByPk> for HashMap<String, String> {
    fn from(aud: user_data::UserDataUsersByPk) -> Self {
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert("created_at".to_string(), aud.created_at.to_string());
        map.insert(
            "enabled_notifications".to_string(),
            aud.enabled_notifications.to_string(),
        );
        map.insert(
            "house_number".to_string(),
            aud.house_number.unwrap_or_default(),
        );
        map.insert("street".to_string(), aud.street.to_string());
        map.insert("chat_id".to_string(), aud.telegram_chat_id.to_string());
        map.insert("street".to_string(), aud.street_by_street.name);

        map
    }
}

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
            dates: Vec::new(),
        }
    }
}

impl From<tomorrow_for_all::TomorrowForAllUsers> for User {
    fn from(au: tomorrow_for_all::TomorrowForAllUsers) -> Self {
        User {
            client_id: au.telegram_chat_id,
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
        write!(f, "{:?}", self.client_id)
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

    #[allow(dead_code)]
    pub async fn get_tomorrows_trash(&self, user_id: i64) -> Result<Vec<TrashDate>> {
        let request_body = TomorrowForUser::build_query(tomorrow_for_user::Variables { user_id });
        let response_data: tomorrow_for_user::ResponseData =
            self.send_request(&request_body).await?;

        Ok(response_data
            .dates
            .into_iter()
            .map(TrashDate::from)
            .collect())
    }

    fn log_errors<T>(&self, response: Response<T>) -> Result<T> {
        if let Some(errors) = &response.errors {
            log::error!("Something failed!");

            for error in errors {
                log::error!("{:?}", error);
            }

            Err(Error::msg(
                "at least one error occurred while querying graphql",
            ))
        } else {
            response
                .data
                .ok_or_else(|| Error::msg("could not extract data from graphql query"))
        }
    }

    pub async fn get_street_id(&self, street_name: String) -> Result<i64> {
        let response_body = SearchStreet::build_query(search_street::Variables {
            limit: Some(1i64),
            name: Some(street_name),
        });
        let result: search_street::ResponseData = self.send_request(&response_body).await?;
        Ok(result.search_streets.into_iter().next().unwrap().id)
    }

    pub async fn get_notification_status(&self, telegram_chat_id: i64) -> Result<bool> {
        let response_body = NotificationStatus::build_query(notification_status::Variables {
            user_id: telegram_chat_id,
        });

        let result: notification_status::ResponseData = self.send_request(&response_body).await?;
        Ok(result
            .users_by_pk
            .ok_or_else(|| Error::msg("user not found"))?
            .enabled_notifications)
    }

    pub async fn get_my_user_data(&self, telegram_chat_id: i64) -> Result<HashMap<String, String>> {
        let response_body = UserData::build_query(user_data::Variables { telegram_chat_id });

        let result = self
            .send_request::<graphql_client::QueryBody<user_data::Variables>, user_data::ResponseData>(&response_body)
            .await?
            .users_by_pk
            .ok_or_else(|| Error::msg("could not find user"))?;
        Ok(HashMap::from(result))
    }

    pub async fn search_similar_streets(&self, street_name: String) -> Result<Vec<Street>> {
        let response_body = SearchStreet::build_query(search_street::Variables {
            limit: Some(5i64),
            name: Some(street_name),
        });
        let result: search_street::ResponseData = self.send_request(&response_body).await?;
        Ok(result
            .search_streets
            .into_iter()
            .map(Street::from)
            .collect())
    }

    pub async fn remove_user_data(&self, telegram_chat_id: i64) -> Result<bool> {
        let response_body = DeleteUser::build_query(delete_user::Variables {
            telegram_chat_id: Some(telegram_chat_id),
        });
        let result: delete_user::ResponseData = self.send_request(&response_body).await?;
        Ok(result
            .delete_users
            .ok_or_else(|| Error::msg("user not found"))?
            .affected_rows
            == 1)
    }

    pub async fn add_user(
        &self,
        telegram_chat_id: i64,
        street: Option<i64>,
        house_number: Option<String>,
    ) -> Result<add_user::ResponseData> {
        let response_body = AddUser::build_query(add_user::Variables {
            telegram_chat_id,
            street,
            house_number,
        });

        self.send_request::<graphql_client::QueryBody<add_user::Variables>, add_user::ResponseData>(
            &response_body,
        )
        .await
    }

    async fn send_request<T: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        json: &T,
    ) -> Result<R> {
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
    }

    pub async fn set_notification(
        &self,
        telegram_chat_id: i64,
        notifications: bool,
    ) -> Result<bool> {
        let response_body = SetNotification::build_query(set_notification::Variables {
            telegram_chat_id,
            enabled_notifications: notifications,
        });

        self.send_request::<graphql_client::QueryBody<set_notification::Variables>, set_notification::ResponseData>(
            &response_body,
        ).await?;
        Ok(notifications)
    }

    #[allow(dead_code)]
    pub async fn get_active_users(&self) -> Result<Vec<User>> {
        let response_body = ActiveUsers::build_query(active_users::Variables {});
        let response_data: active_users::ResponseData = self.send_request(&response_body).await?;

        Ok(response_data.users.into_iter().map(User::from).collect())
    }

    pub async fn get_active_users_tomorrow(&self) -> Result<Vec<User>> {
        let request_body = TomorrowForAll::build_query(tomorrow_for_all::Variables {});
        let response_data: tomorrow_for_all::ResponseData =
            self.send_request(&request_body).await?;

        Ok(response_data.users.into_iter().map(User::from).collect())
    }
}
