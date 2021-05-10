use anyhow::Error;
use chrono::NaiveDate;
use graphql_client::{GraphQLQuery, Response};
use lazy_static::lazy_static;
use maplit::hashmap;
use std::collections::HashMap;
use std::env;
use std::hash::Hash;

static ENDPOINT: &str = "http://192.168.0.207:9123/v1/graphql";
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

#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/tomorrow.graphql",
    response_derives = "Debug",
    normalization = "rust"
)]
pub struct TrashAtDate;

fn build_objects_from_query(trash_response: trash_at_date::ResponseData) -> Vec<TrashDate> {
    let mut data: Vec<TrashDate> = Vec::with_capacity(trash_response.dates.len());
    // let mut data: Vec<TrashDate> = Vec::new();

    for res in trash_response.dates {
        let possible_trash_type = TRASH_MAP.get(&res.trash_type_by_trash_type.name[..]);
        if let Some(t) = possible_trash_type {
            data.push(TrashDate {
                date: res.date,
                trash_type: t.clone(),
                name: res.trash_type_by_trash_type.name,
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

pub async fn get_tomorrows_trash() -> Result<Vec<TrashDate>, Error> {
    let secret = env::var("HASURA_SECRET")
        .expect("Hasura header authorisation not set, set env variable 'HASURA_SECRET'.");
    // let variables: trash_at_date::Variables();
    let request_body = TrashAtDate::build_query(trash_at_date::Variables {});

    let client = reqwest::Client::new();
    let response = client
        .post(ENDPOINT)
        .header(HASURA_HEADER, secret)
        .json(&request_body)
        .send()
        .await?;

    let response_body: Response<trash_at_date::ResponseData> = response.json().await?;

    if let Some(errors) = response_body.errors {
        log::error!("Something failed!");

        for error in &errors {
            log::error!("{:?}", error);
        }
    }

    let response_data: trash_at_date::ResponseData = response_body.data.expect("no response data");

    Ok(build_objects_from_query(response_data))
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
