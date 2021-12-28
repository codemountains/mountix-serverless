use aws_sdk_dynamodb::model::{AttributeValue, Select};
use aws_sdk_dynamodb::Client;
use std::collections::HashMap;

pub struct ScanCommand {
    pub(crate) table: String,
}

pub struct QueryCommand {
    pub(crate) table: String,
    pub(crate) index: Option<String>,
    pub(crate) key: String,
    pub(crate) value: String,
}

pub struct QueryFilterCommand {
    pub(crate) table: String,
    pub(crate) index: Option<String>,
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) filter_key: String,
    pub(crate) filter_value: String,
}

pub async fn scan_all(
    client: &Client,
    command: ScanCommand,
) -> Result<Vec<HashMap<String, AttributeValue>>, ()> {
    match client.scan().table_name(command.table).send().await {
        Ok(resp) => {
            match resp.items {
                Some(items) => Ok(items),
                _ => Err(()),
            }
            // if resp.count > 0 {
            //     match resp.items {
            //         Some(items) => Ok(items),
            //         _ => Err(()),
            //     }
            // } else {
            //     let empty_list: Vec<HashMap<String, AttributeValue>> = vec![];
            //     Ok(empty_list)
            // }
        }
        Err(_) => Err(()),
    }
}

pub async fn query(
    client: &Client,
    command: QueryCommand,
) -> Result<Vec<HashMap<String, AttributeValue>>, ()> {
    let key = &command.key;
    let value = &command.value;

    match client
        .query()
        .table_name(command.table)
        .key_condition_expression("#key = :value".to_string())
        .expression_attribute_names("#key".to_string(), key.to_string())
        .expression_attribute_values(":value".to_string(), AttributeValue::N(value.to_string()))
        .scan_index_forward(true)
        .select(Select::AllAttributes)
        .send()
        .await
    {
        Ok(resp) => {
            // match resp.items {
            //     Some(items) => Ok(items),
            //     _ => Err(()),
            // }
            if resp.count > 0 {
                match resp.items {
                    Some(items) => Ok(items),
                    _ => Err(()),
                }
            } else {
                Err(())
            }
        }
        Err(_) => Err(()),
    }
}

pub async fn query_index(
    client: &Client,
    command: QueryCommand,
) -> Result<Vec<HashMap<String, AttributeValue>>, ()> {
    let key = &command.key;
    let value = &command.value;

    let mut index = String::new();
    match command.index {
        Some(param_index) => {
            index = param_index;
        }
        _ => {}
    }

    match client
        .query()
        .table_name(command.table)
        .index_name(index)
        .key_condition_expression("#key = :value".to_string())
        .expression_attribute_names("#key".to_string(), key.to_string())
        .expression_attribute_values(":value".to_string(), AttributeValue::S(value.to_string()))
        .scan_index_forward(true)
        .select(Select::AllAttributes)
        .send()
        .await
    {
        Ok(resp) => match resp.items {
            Some(items) => Ok(items),
            _ => Err(()),
        },
        Err(_) => Err(()),
    }
}

pub async fn query_index_filter(
    client: &Client,
    command: QueryFilterCommand,
) -> Result<Vec<HashMap<String, AttributeValue>>, ()> {
    let key = &command.key;
    let value = &command.value;
    let filter_key = &command.filter_key;
    let filter_value = &command.filter_value;

    let mut index = String::new();
    match command.index {
        Some(param_index) => {
            index = param_index;
        }
        _ => {}
    }

    match client
        .query()
        .table_name(command.table)
        .index_name(index)
        .key_condition_expression("#key = :value".to_string())
        .expression_attribute_names("#key".to_string(), key.to_string())
        .expression_attribute_values(":value".to_string(), AttributeValue::S(value.to_string()))
        .filter_expression("contains(#filterKey, :filterKey)".to_string())
        .expression_attribute_names("#filterKey".to_string(), filter_key.to_string())
        .expression_attribute_values(
            ":filterKey".to_string(),
            AttributeValue::S(filter_value.to_string()),
        )
        .scan_index_forward(true)
        .select(Select::AllAttributes)
        .send()
        .await
    {
        Ok(resp) => match resp.items {
            Some(items) => Ok(items),
            _ => Err(()),
        },
        Err(_) => Err(()),
    }
}
