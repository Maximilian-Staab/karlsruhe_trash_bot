use anyhow::Error;
use chrono::NaiveDate;
use graphql_client::{GraphQLQuery, Response};
use lazy_static::lazy_static;
use maplit::hashmap;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::fmt::Formatter;
use std::hash::Hash;

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
    query_path = "graphql/tomorrow.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct TrashAtDate;

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

impl From<active_users::ActiveUsersUsers> for User {
    fn from(au: active_users::ActiveUsersUsers) -> Self {
        User {
            client_id: au.telegram_chat_id,
            first_name: au.first_name.unwrap_or("".to_string()),
            last_name: au.last_name.unwrap_or("".to_string()),
        }
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

    fn build_objects_from_query(
        &self,
        trash_response: &trash_at_date::ResponseData,
    ) -> Vec<TrashDate> {
        let mut data: Vec<TrashDate> = Vec::with_capacity(trash_response.dates.len());
        // let mut data: Vec<TrashDate> = Vec::new();

        for res in &trash_response.dates[..] {
            let possible_trash_type = TRASH_MAP.get(&res.trash_type_by_trash_type.name[..]);
            if let Some(t) = possible_trash_type {
                data.push(TrashDate {
                    date: res.date,
                    trash_type: t.clone(),
                    name: res.trash_type_by_trash_type.name.to_owned(),
                });
            } else {
                log::warn!(
                    "Could not find the selected type of trash: {}",
                    &res.trash_type_by_trash_type.name[..]
                );
            }
        }

        data
    }

    pub async fn get_tomorrows_trash(&self) -> Result<Vec<TrashDate>, Error> {
        // let variables: trash_at_date::Variables();
        let request_body = TrashAtDate::build_query(trash_at_date::Variables {});

        let response = self
            .client
            .post(&self.endpoint)
            .header(HASURA_HEADER, &self.secret)
            .json(&request_body)
            .send()
            .await?;

        let response_body: Response<trash_at_date::ResponseData> = response.json().await?;

        self.log_errors(&response_body);

        let response_data: trash_at_date::ResponseData =
            response_body.data.expect("no response data");

        Ok(self.build_objects_from_query(&response_data))
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

lazy_static! {
    static ref TRASH_MAP: HashMap<&'static str, TrashType> = hashmap! {
        "Bioabfall" => TrashType::Organic,
        "Wertstoff"=> TrashType::Recycling,
        "Papier"=> TrashType::Paper,
        "Restmüll"=> TrashType::Miscellaneous,
    };
}

lazy_static! {
    static ref TRASH_TO_STRING: HashMap<TrashType, &'static str> = hashmap! {
        TrashType::Organic => "Bioabfall",
        TrashType::Recycling => "Wertstoff",
        TrashType::Paper => "Papier",
        TrashType::Miscellaneous => "Restmüll",
    };
}

#[derive(Debug, Clone)]
pub struct TrashDate {
    pub date: NaiveDate,
    pub trash_type: TrashType,
    pub name: String,
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
