use std::fmt::{Display, Formatter};

use anyhow::{Error, Result};
use geocoding::{DetailedReverse, Openstreetmap, Point};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;

type Responder<T> = oneshot::Sender<Result<T, Error>>;

#[derive(Debug)]
pub struct Lookup {
    pub longitude: f32,
    pub latitude: f32,

    pub responder: Responder<Option<LocationResult>>,
}

impl Display for Lookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Lookup {{ Longitude: {}, Latitude: {} }}",
            self.longitude.to_string(),
            self.latitude.to_string()
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationResult {
    pub street: String,
    pub house_number: Option<String>,
    pub city: String,
    pub country: String,
}

impl Display for LocationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let r = write!(f, "{}, {}", self.city, self.street);
        if let Some(number) = &self.house_number {
            return write!(f, " {}", number);
        }
        return r;
    }
}

pub struct LocationLookup {
    receiver: Receiver<Lookup>,
}

impl LocationLookup {
    pub async fn new(receiver: Receiver<Lookup>) -> LocationLookup {
        LocationLookup { receiver }
    }

    // async fn request(&self, longitude: f32, latitude: f32) -> Result<AddressDetails, Error> {
    //     let response = self
    //         .client
    //         .get(&format!("{}reverse", self.endpoint))
    //         .query(&[
    //             (&"lon", &longitude.to_string()),
    //             (&"lat", &latitude.to_string()),
    //             (&"format", &String::from("geojson")),
    //         ])
    //         .send()
    //         .await?
    //         .error_for_status()?;
    //
    //     let mut result: OpenstreetmapResponse<f32> = response.json().await?;
    //     let address = result.features.pop().unwrap().properties.address.unwrap();
    //     Ok(address)
    // }

    pub async fn start(&mut self) {
        log::info!("Starting Lookup Service");
        while let Some(lookup) = self.receiver.recv().await {
            log::info!("Got Lookup Request: {}", lookup);

            let longitude = lookup.longitude.clone();
            let latitude = lookup.latitude.clone();
            let result = tokio::task::spawn_blocking(move || {
                Openstreetmap::new().detailed_reverse(&Point::new(longitude, latitude))
            })
            .await
            .expect("Task didn't finish.");

            if let Err(e) = result {
                lookup.responder.send(Err(Error::from(e))).unwrap();
            } else {
                if let Some(address) = result.unwrap() {
                    let result = LocationResult {
                        city: address.city.unwrap_or("".to_string()),
                        country: address.country.unwrap_or("".to_string()),
                        house_number: match address.house_number {
                            None => None,
                            Some(n) => Some(n.parse().unwrap()),
                        },
                        street: address.road.unwrap_or("".to_string()),
                    };
                    log::info!("Found location: {}", result);
                    lookup.responder.send(Ok(Some(result))).unwrap();
                } else {
                    log::warn!("Didn't find anything: {}", lookup);
                    lookup.responder.send(Ok(None)).unwrap();
                }
            }

            tokio::time::sleep(Duration::from_secs(1u64)).await;
        }
        log::info!("Stopping Lookup Service");
    }
}
