pub mod trash {
    use anyhow::Error;
    use chrono::NaiveDate;
    use graphql_client::{GraphQLQuery, Response};
    use lazy_static::lazy_static;
    use maplit::hashmap;
    use reqwest;
    use std::collections::HashMap;
    use std::env;
    use std::hash::Hash;
    use std::str::FromStr;

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
        // let mut data: Vec<TrashDate> = Vec::with_capacity(trash_response.dates.length());
        let mut data: Vec<TrashDate> = Vec::new();
        for res in trash_response.dates {
            data.push(TrashDate {
                date: res.date,
                trash_type: TRASH_MAP
                    .get(&res.trash_type_by_trash_type.name[..])
                    .expect("Name not found.")
                    .clone(),
                name: res.trash_type_by_trash_type.name,
            })
        }

        data
    }

    pub async fn perform_my_query() -> Result<Vec<TrashDate>, Error> {
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
        // let result = response_body.data.expect("No thing today.");
        // for data in result.dates {
        //     println!("{:?}", data);
        // }

        if let Some(errors) = response_body.errors {
            log::error!("Something failed!");

            for error in &errors {
                log::error!("{:?}", error);
            }
        }

        let response_data: trash_at_date::ResponseData =
            response_body.data.expect("no response data");

        println!("{:?}", response_data);
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

    impl TrashDate {
        pub fn new(trash_type: TrashType, date_string: &str) -> Result<Self, InvalidDateError> {
            let date = match NaiveDate::from_str(date_string) {
                Ok(d) => d,
                Err(_) => return Err(InvalidDateError),
            };

            let name = String::from(
                *TRASH_TO_STRING
                    .get(&trash_type)
                    .expect("Invalid trash string."),
            );

            Ok(TrashDate {
                date,
                trash_type,
                name,
            })
        }
    }
}
