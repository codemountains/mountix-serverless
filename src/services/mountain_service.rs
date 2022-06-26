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

pub struct RangeCondition {
    pub offset: usize,
    pub limit: Option<usize>,
}

struct MountainData {
    index: String,
    attribute_data_list: Vec<HashMap<String, AttributeValue>>,
}

pub struct SearchedMountainResult {
    pub mountains: Vec<Mountain>,
    pub total: usize,
    pub offset: usize,
    pub limit: Option<usize>,
}

pub async fn get_all_mountains(
    client: &Client,
    range_condition: RangeCondition,
    sort_key: &String,
) -> Result<SearchedMountainResult, ()> {
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
                let mapper = MountainMapper::new(mountain_data.attribute_data_list);
                mountains.push(mapper.to_mountain());
            }

            // sorting
            if mountains.len() > 0 {
                sort_mountains(&mut mountains, sort_key);
            }

            // offset, limitによる絞り込み
            match refine_mountains(&mountains, range_condition) {
                Ok(refined_mountain_result) => Ok(SearchedMountainResult {
                    mountains: refined_mountain_result.mountains,
                    total: refined_mountain_result.total,
                    offset: refined_mountain_result.offset,
                    limit: refined_mountain_result.limit,
                }),
                Err(_) => Err(()),
            }
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
            let mapper = MountainMapper::new(response);
            Ok(mapper.to_mountain())
        }
        Err(_) => Err(()),
    }
}

pub async fn search_mountains(
    client: &Client,
    search_conditions: Vec<SearchCondition>,
    range_condition: RangeCondition,
    sort_key: &String,
) -> Result<SearchedMountainResult, ()> {
    // 検索結果を格納する
    let mut searched_list: Vec<String> = Vec::new();

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

        let filter_kana_command = QueryFilterCommand {
            table: "Mountains".to_string(),
            index: Some("DataType_Id_Index".to_string()),
            key: "DataType".to_string(),
            value: "NameKana".to_string(),
            filter_key: "DataValue".to_string(),
            filter_value: condition.value.to_string(),
        };

        let key = String::from("Id");

        match condition.search_type {
            SearchType::Prefecture | SearchType::Tag => match query_index(client, command).await {
                Ok(response) => {
                    let mut temp_result: Vec<String> = Vec::new();
                    for item in response {
                        let id = get_value(&item, &key, ValueType::Number);
                        temp_result.push(id);
                    }

                    merge_result(&mut searched_list, &temp_result);
                }
                Err(_) => {}
            },
            SearchType::Name => {
                let mut temp_name_result: Vec<String> = Vec::new();
                let mut temp_kana_result: Vec<String> = Vec::new();

                match query_index_filter(client, filter_command).await {
                    Ok(response) => {
                        for item in response {
                            let id = get_value(&item, &key, ValueType::Number);
                            temp_name_result.push(id);
                        }
                    }
                    Err(_) => {}
                }
                match query_index_filter(client, filter_kana_command).await {
                    Ok(response) => {
                        for item in response {
                            if temp_name_result.len() > 0 {
                                for n in &temp_name_result {
                                    let id = get_value(&item, &key, ValueType::Number);
                                    if id != n.to_string() {
                                        temp_kana_result.push(id);
                                    }
                                }
                            } else {
                                let id = get_value(&item, &key, ValueType::Number);
                                temp_kana_result.push(id);
                            }
                        }

                        for k in &temp_kana_result {
                            temp_name_result.push(k.to_string());
                        }
                    }
                    Err(_) => {}
                }

                merge_result(&mut searched_list, &temp_name_result);
            }
        }
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

    // sorting
    if mountains.len() > 0 {
        sort_mountains(&mut mountains, sort_key);
    }

    // offset, limitによる絞り込み
    match refine_mountains(&mountains, range_condition) {
        Ok(refined_mountain_result) => Ok(SearchedMountainResult {
            mountains: refined_mountain_result.mountains,
            total: refined_mountain_result.total,
            offset: refined_mountain_result.offset,
            limit: refined_mountain_result.limit,
        }),
        Err(_) => Err(()),
    }
}

fn merge_result(base_list: &mut Vec<String>, target_list: &Vec<String>) {
    if base_list.len() > 0 {
        let mut keep: Vec<bool> = Vec::new();
        for base_id in base_list.into_iter() {
            let mut is_duplicated = false;
            for target_id in target_list {
                if base_id == target_id {
                    is_duplicated = true;
                }
            }
            keep.push(is_duplicated);
        }

        let mut iter = keep.iter();
        base_list.retain(|_| *iter.next().unwrap());
    } else {
        for target_id in target_list {
            base_list.push(target_id.to_string());
        }
    }
}

struct RefinedMountainResult {
    mountains: Vec<Mountain>,
    total: usize,
    offset: usize,
    limit: Option<usize>,
}

fn refine_mountains(
    mountains: &Vec<Mountain>,
    range_condition: RangeCondition,
) -> Result<RefinedMountainResult, String> {
    let range_from = range_condition.offset;
    let mut range_to = mountains.len() as usize;
    match range_condition.limit {
        Some(range_condition_limit) => {
            if range_to > range_condition_limit + range_from {
                range_to = range_condition_limit + range_from;
            }
        }
        None => {}
    }

    if range_from > range_to {
        return Err("offsetの値が不正です。".to_string());
    }

    Ok(RefinedMountainResult {
        mountains: mountains[range_from..range_to].to_vec(),
        total: mountains.len(),
        offset: range_condition.offset,
        limit: range_condition.limit,
    })
}

fn sort_mountains(mountains: &mut Vec<Mountain>, sort_key: &String) {
    match sort_key.as_str() {
        "id.asc" => {
            mountains.sort_by(|a, b| a.id.cmp(&b.id));
        }
        "id.desc" => {
            mountains.sort_by(|a, b| b.id.cmp(&a.id));
        }
        "elevation.asc" => {
            mountains.sort_by(|a, b| a.elevation.cmp(&b.elevation));
        }
        "elevation.desc" => {
            mountains.sort_by(|a, b| b.elevation.cmp(&a.elevation));
        }
        "name.asc" => {
            mountains.sort_by(|a, b| a.name_kana.cmp(&b.name_kana));
        }
        "name.desc" => {
            mountains.sort_by(|a, b| b.name_kana.cmp(&a.name_kana));
        }
        _ => {
            mountains.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
}
