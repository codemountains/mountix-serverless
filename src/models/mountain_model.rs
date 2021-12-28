use aws_sdk_dynamodb::model::AttributeValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait MountainBaseMapper {
    fn new(data: Vec<HashMap<String, AttributeValue>>) -> Self;
    fn to_mountain(&self) -> Mountain;
}

pub struct MountainMapper {
    data: Vec<HashMap<String, AttributeValue>>,
}

pub enum ValueType {
    String,
    Number,
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mountain {
    pub(crate) id: u32,
    name: String,
    name_kana: String,
    area: String,
    prefectures: Vec<String>,
    elevation: u32,
    location: Location,
    tags: Vec<String>,
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    latitude: f64,
    longitude: f64,
    gsi_url: String,
}

impl MountainBaseMapper for MountainMapper {
    fn new(data: Vec<HashMap<String, AttributeValue>>) -> Self {
        Self { data }
    }

    fn to_mountain(&self) -> Mountain {
        let mut mountain = Mountain {
            id: 0,
            name: "".to_string(),
            name_kana: "".to_string(),
            area: "".to_string(),
            prefectures: vec![],
            elevation: 0,
            location: Location {
                latitude: 0.0,
                longitude: 0.0,
                gsi_url: "".to_string(),
            },
            tags: vec![],
        };

        match self.data.get(0) {
            Some(top_data) => {
                let key = String::from("Id");
                match get_value(top_data, &key, ValueType::Number).parse() {
                    Ok(id) => mountain.id = id,
                    Err(_) => {}
                };
            }
            _ => {}
        }

        for item in &self.data {
            match item.get(&*"DataType") {
                Some(type_attr) => match type_attr.as_s() {
                    Ok(data_type) => match data_type.as_str() {
                        "Name" => {
                            let key = String::from("DataValue");
                            mountain.name = get_value(item, &key, ValueType::String);
                        }
                        "NameKana" => {
                            let key = String::from("DataValue");
                            mountain.name_kana = get_value(item, &key, ValueType::String);
                        }
                        "Elevation" => {
                            let key = String::from("ElevationValue");
                            if let Ok(elevation_value) =
                                get_value(item, &key, ValueType::Number).parse::<u32>()
                            {
                                mountain.elevation = elevation_value;
                            }
                        }
                        "Location" => {
                            mountain.location = get_location(item);
                        }
                        _ => {
                            let key = String::from("DataValue");
                            if Some(0) == data_type.find("Area_") {
                                mountain.area =
                                    get_value(item, &key, ValueType::String).replace("Area_", "");
                            } else if Some(0) == data_type.find("Prefecture_") {
                                mountain.prefectures.push(
                                    get_value(item, &key, ValueType::String)
                                        .replace("Prefecture_", ""),
                                );
                            } else if Some(0) == data_type.find("Tag_") {
                                mountain.tags.push(
                                    get_value(item, &key, ValueType::String).replace("Tag_", ""),
                                );
                            }
                        }
                    },
                    Err(_) => {}
                },
                _ => {}
            }
        }

        mountain
    }
}

pub fn get_value(
    item: &HashMap<String, AttributeValue>,
    key: &String,
    value_type: ValueType,
) -> String {
    match item.get(key) {
        Some(attr_value) => match value_type {
            ValueType::String => match attr_value.as_s() {
                Ok(data_value) => data_value.to_string(),
                Err(_) => "".to_string(),
            },
            ValueType::Number => match attr_value.as_n() {
                Ok(data_value) => data_value.to_string(),
                Err(_) => "".to_string(),
            },
        },
        _ => "".to_string(),
    }
}

fn get_location(location_value: &HashMap<String, AttributeValue>) -> Location {
    let mut location = Location {
        latitude: 0.0,
        longitude: 0.0,
        gsi_url: "".to_string(),
    };

    match location_value.get(&*"LocationValue") {
        Some(value_attr) => match value_attr.as_m() {
            Ok(data_value) => {
                let lat_key = String::from("Latitude");
                let lat_value = get_value(data_value, &lat_key, ValueType::Number);
                if let Ok(latitude) = lat_value.parse::<f64>() {
                    location.latitude = latitude;
                }

                let lon_key = String::from("Longitude");
                let lon_value = get_value(data_value, &lon_key, ValueType::Number);
                if let Ok(longitude) = lon_value.parse::<f64>() {
                    location.longitude = longitude;
                }

                let url_key = String::from("GsiUrl");
                location.gsi_url = get_value(data_value, &url_key, ValueType::String);
            }
            Err(_) => {}
        },
        _ => {}
    }

    location
}
