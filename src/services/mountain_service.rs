use crate::models::{get_value, Mountain, MountainBaseMapper, MountainMapper, ValueType};
use crate::repositories::{
    query, query_index, query_index_filter, scan_all, QueryCommand, QueryFilterCommand, ScanCommand,
};
use aws_sdk_dynamodb::model::AttributeValue;
use aws_sdk_dynamodb::Client;
use std::collections::HashMap;

pub enum SearchType {
    Name,
    Prefecture,
    Tag,
}

pub struct SearchCondition {
    pub search_type: SearchType,
    pub value: String,
}

struct MountainData {
    index: String,
    attribute_data_list: Vec<HashMap<String, AttributeValue>>,
}

pub async fn get_all_mountains(client: &Client) -> Result<Vec<Mountain>, ()> {
    let command = ScanCommand {
        table: "Mountains".to_string(),
    };

    match scan_all(client, command).await {
        Ok(response) => {
            // id毎に切り分けたデータを格納する
            let mut mountain_data_list: Vec<MountainData> = Vec::new();
            for item in response {
                match item.get("Id") {
                    Some(attr_value) => match attr_value.as_n() {
                        Ok(id) => {
                            let mut is_duplicated = false;
                            let mut target_index = 0u32;
                            for m_data in &mountain_data_list {
                                if m_data.index == id.to_string() {
                                    is_duplicated = true;
                                    break;
                                }
                                target_index = target_index + 1;
                            }

                            if !is_duplicated {
                                mountain_data_list.push(MountainData {
                                    index: id.to_string(),
                                    attribute_data_list: vec![item],
                                });
                            } else {
                                mountain_data_list[target_index as usize]
                                    .attribute_data_list
                                    .push(item);
                            }
                        }
                        Err(_) => {}
                    },
                    _ => {}
                }
            }

            let mut mountains: Vec<Mountain> = Vec::new();
            for mountain_data in mountain_data_list {
                // let mapper = MountainMapper {
                //     data: mountain_data.attribute_data_list,
                // };
                let mapper = MountainMapper::new(mountain_data.attribute_data_list);
                mountains.push(mapper.to_mountain());
            }

            if mountains.len() > 0 {
                mountains.sort_by(|a, b| a.id.cmp(&b.id));
            }
            Ok(mountains)
        }
        Err(_) => Err(()),
    }
}

pub async fn get_mountain_by_id(client: &Client, id: String) -> Result<Mountain, ()> {
    let command = QueryCommand {
        table: "Mountains".to_string(),
        index: None,
        key: "Id".to_string(),
        value: id.to_string(),
    };

    match query(client, command).await {
        Ok(response) => {
            // let mapper = MountainMapper { data: response };
            let mapper = MountainMapper::new(response);
            Ok(mapper.to_mountain())
        }
        Err(_) => Err(()),
    }
}

pub async fn search_mountains(
    client: &Client,
    search_conditions: Vec<SearchCondition>,
) -> Result<Vec<Mountain>, ()> {
    // 各検索結果を格納する
    let mut pref_searched_list: Vec<String> = Vec::new();
    let mut tag_searched_list: Vec<String> = Vec::new();
    let mut name_searched_list: Vec<String> = Vec::new();

    for condition in search_conditions {
        let command = QueryCommand {
            table: "Mountains".to_string(),
            index: Some("DataValue_Id_Index".to_string()),
            key: "DataValue".to_string(),
            value: condition.value.to_string(),
        };

        let filter_command = QueryFilterCommand {
            table: "Mountains".to_string(),
            index: Some("DataType_Id_Index".to_string()),
            key: "DataType".to_string(),
            value: "Name".to_string(),
            filter_key: "DataValue".to_string(),
            filter_value: condition.value.to_string(),
        };

        let key = String::from("Id");

        match condition.search_type {
            SearchType::Prefecture => match query_index(client, command).await {
                Ok(response) => {
                    for item in response {
                        let id = get_value(&item, &key, ValueType::Number);
                        pref_searched_list.push(id);
                    }
                }
                Err(_) => {}
            },
            SearchType::Tag => match query_index(client, command).await {
                Ok(response) => {
                    for item in response {
                        let id = get_value(&item, &key, ValueType::Number);
                        tag_searched_list.push(id);
                    }
                }
                Err(_) => {}
            },
            SearchType::Name => match query_index_filter(client, filter_command).await {
                Ok(response) => {
                    for item in response {
                        let id = get_value(&item, &key, ValueType::Number);
                        name_searched_list.push(id);
                    }
                }
                Err(_) => {}
            },
        }
    }

    // 各検索結果から重複する結果のみを取得する
    let mut searched_list: Vec<String> = Vec::new();
    if pref_searched_list.len() > 0 && tag_searched_list.len() > 0 && name_searched_list.len() > 0 {
        for pref_searched_id in &pref_searched_list {
            for tag_searched_id in &tag_searched_list {
                for name_searched_id in &name_searched_list {
                    if pref_searched_id == tag_searched_id && pref_searched_id == name_searched_id {
                        searched_list.push(pref_searched_id.to_string());
                    }
                }
            }
        }
    } else if pref_searched_list.len() > 0 && tag_searched_list.len() > 0 {
        searched_list = get_duplication_list(&pref_searched_list, &tag_searched_list);
    } else if pref_searched_list.len() > 0 && name_searched_list.len() > 0 {
        searched_list = get_duplication_list(&pref_searched_list, &name_searched_list);
    } else if tag_searched_list.len() > 0 && name_searched_list.len() > 0 {
        searched_list = get_duplication_list(&tag_searched_list, &name_searched_list);
    } else if pref_searched_list.len() > 0 {
        searched_list = pref_searched_list;
    } else if tag_searched_list.len() > 0 {
        searched_list = tag_searched_list;
    } else if name_searched_list.len() > 0 {
        searched_list = name_searched_list;
    }

    let mut mountains: Vec<Mountain> = Vec::new();
    for id in searched_list {
        match get_mountain_by_id(client, id).await {
            Ok(mountain) => {
                mountains.push(mountain);
            }
            Err(_) => {}
        }
    }

    // if mountains.len() > 0 {
    //     Ok(mountains)
    // } else {
    //     Err(())
    // }
    Ok(mountains)
}

fn get_duplication_list(a: &Vec<String>, b: &Vec<String>) -> Vec<String> {
    let mut result_list: Vec<String> = Vec::new();
    for a_id in a {
        for b_id in b {
            if a_id == b_id {
                result_list.push(a_id.to_string());
            }
        }
    }
    result_list
}