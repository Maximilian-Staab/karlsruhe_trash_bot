use std::env;
use std::fmt::Formatter;
use std::hash::Hash;

use anyhow::Error;
use chrono::NaiveDate;
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;

static HASURA_HEADER: &str = "x-hasura-admin-secret";

#[derive(Debug)]
pub struct InvalidDateError;

type Date = chrono::NaiveDate;

#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub enum TrashType {
    Organic,
    Recycling,
    Paper,
    Miscellaneous,
}

#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub struct User {
    first_name: String,
    last_name: String,
    pub client_id: i64,
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
    query_path = "graphql/users.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct ActiveUsers;

#[derive(Debug, Clone)]
pub struct RequestPerformer {
    secret: String,
    endpoint: String,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct TrashDate {
    pub date: NaiveDate,
    pub trash_type: TrashType,
    pub name: String,
}

impl From<active_users::ActiveUsersUsers> for User {
    fn from(au: active_users::ActiveUsersUsers) -> Self {
        User {
            client_id: au.telegram_chat_id,
            first_name: au.first_name.unwrap_or_else(|| "".to_string()),
            last_name: au.last_name.unwrap_or_else(|| "".to_string()),
        }
    }
}

impl From<tomorrow_for_user::TomorrowForUserDates> for TrashDate {
    fn from(tat: tomorrow_for_user::TomorrowForUserDates) -> Self {
        let trash_type = match &tat.trash_type_by_trash_type.name[..] {
            "Bioabfall" => TrashType::Organic,
            "Wertstoff" => TrashType::Recycling,
            "Papier" => TrashType::Paper,
            "Restmüll" => TrashType::Miscellaneous,
            _ => panic!(
                "Could not find the selected type of trash: {}",
                &tat.trash_type_by_trash_type.name[..]
            ),
        };

        TrashDate {
            name: tat.trash_type_by_trash_type.name,
            date: tat.date,
            trash_type,
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
            "{} {}: {}",
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

        let response = self
            .client
            .post(&self.endpoint)
            .header(HASURA_HEADER, &self.secret)
            .json(&request_body)
            .send()
            .await?;

        let response_body: Response<tomorrow_for_user::ResponseData> = response.json().await?;

        self.log_errors(&response_body);

        Ok(response_body
            .data
            .expect("no response data")
            .dates
            .into_iter()
            .map(TrashDate::from)
            .collect())
    }

    fn log_errors<T>(&self, response: &Response<T>) {
        if let Some(errors) = &response.errors {
            log::error!("Something failed!");

            for error in errors {
                log::error!("{:?}", error);
            }
        }
    }

    pub async fn get_active_users(&self) -> Result<Vec<User>, Error> {
        let response_body = ActiveUsers::build_query(active_users::Variables {});

        let response = self
            .client
            .post(&self.endpoint)
            .header(HASURA_HEADER, &self.secret)
            .json(&response_body)
            .send()
            .await?;

        let response_body: Response<active_users::ResponseData> = response.json().await?;

        self.log_errors(&response_body);

        let response_data: active_users::ResponseData =
            response_body.data.expect("no response data");

        Ok(response_data.users.into_iter().map(User::from).collect())
    }
}
