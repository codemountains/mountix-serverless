use aws_sdk_dynamodb::Client;
use lambda_http::{
    handler,
    lambda_runtime::{self, Context},
    IntoResponse, Request, RequestExt, Response, StrMap,
};
use mountix_serverless::models::{
    PrefectureBaseMapper, PrefectureMapper, TagBaseMapper, TagMapper,
};
use mountix_serverless::services;
use mountix_serverless::services::{SearchCondition, SearchType, RangeCondition};
use serde::{Deserialize, Serialize};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

enum ResponseType {
    ApiInfo,
    Mountain,
    MountainList,
    Error,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(handler(get_response)).await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrResponse {
    messages: Vec<String>,
}

async fn get_response(event: Request, _: Context) -> Result<impl IntoResponse, Error> {
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);

    let uri_path = event.uri().path();
    let query_params = event.query_string_parameters();

    let mut mountain_id = "".to_string();
    if let Some(id) = event.path_parameters().get("id") {
        mountain_id = id.to_string();
    }

    let mut json = format!(
        r#"{{"about": "{}", "mountains": "{}", "documents": "{}"}}"#,
        "日本の主な山岳をJSON形式で提供するAPIです。",
        "https://mountix.codemountains.org/api/v1/mountains",
        "https://mountix-docs.codemountains.org/"
    );
    let mut status = 200;
    match response_type(&uri_path.to_string(), &mountain_id) {
        ResponseType::ApiInfo => {}
        ResponseType::Mountain => match get_mountain(&client, &mountain_id).await {
            Ok(result) => {
                json = result;
            }
            Err(_) => {
                status = 404;
                json = format!(r#"{{"message": "{}"}}"#, "山岳情報が見つかりませんでした。");
            }
        },
        ResponseType::MountainList => match search_mountains(&client, &query_params).await {
            Ok(result) => {
                let mut limit = "null".to_string();
                if let Some(l) = result.limit {
                    limit = l.to_string();
                }

                json = format!(
                    r#"{{"mountains": {}, "total": {}, "offset": {}, "limit": {}}}"#,
                    result.mountains_json, result.total, result.offset, limit
                );
            }
            Err(e) => {
                status = 400;
                let err_messages = ErrResponse { messages: e };
                match serde_json::to_string_pretty(&err_messages) {
                    Ok(msg) => json = msg,
                    Err(_) => {
                        json = format!(
                            r#"{{"message": "{}"}}"#,
                            "エラーが発生しました。".to_string()
                        )
                    }
                }
            }
        },
        ResponseType::Error => {
            status = 400;
            json = format!(r#"{{"message": "{}"}}"#, uri_path.to_string());
        }
    }

    // エラーのレスポンスをOkで実装する
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Methods", "OPTIONS,GET")
        .header("Access-Control-Allow-Credential", "true")
        .header("Access-Control-Allow-Origin", "*")
        .body(json)
        .expect("failed to render response"))
}

fn response_type(uri_path: &String, mountain_id: &String) -> ResponseType {
    match uri_path.to_string().replace("/api/v1", "").as_str() {
        "" | "/" => ResponseType::ApiInfo,
        "/mountains" | "/mountains/" => ResponseType::MountainList,
        _ => match mountain_id.to_string().parse::<u32>() {
            Ok(_) => ResponseType::Mountain,
            Err(_) => ResponseType::Error,
        },
    }
}

async fn get_mountain(client: &Client, id: &String) -> Result<String, ()> {
    match services::get_mountain_by_id(&client, id.to_string()).await {
        Ok(mountain) => match serde_json::to_string_pretty(&mountain) {
            Ok(result) => Ok(result),
            Err(_) => Err(()),
        },
        Err(_) => Err(()),
    }
}

struct SearchedResult {
    mountains_json: String,
    total: usize,
    offset: usize,
    limit: Option<usize>,
}

async fn search_mountains(
    client: &Client,
    query_params: &StrMap,
) -> Result<SearchedResult, Vec<String>> {
    let simple_err_message_list = vec!["エラーが発生しました。".to_string()];

    let mut search_conditions: Vec<SearchCondition> = Vec::new();
    let mut err_message_list: Vec<String> = Vec::new();

    // 検索条件: 都道府県ID
    if let Some(pref) = query_params.get("prefecture") {
        if let Ok(pref_key) = pref.to_string().parse::<u32>() {
            let pref_mapper = PrefectureMapper::new(pref_key);
            match pref_mapper.to_prefecture() {
                Ok(pref) => {
                    search_conditions.push(SearchCondition {
                        search_type: SearchType::Prefecture,
                        value: pref,
                    });
                }
                Err(_) => {
                    err_message_list.push("不正な都道府県IDです".to_string());
                }
            }
        }
    }

    // 検索条件: タグ（百名山）
    if let Some(tag) = query_params.get("tag") {
        if let Ok(tag_key) = tag.to_string().parse::<u32>() {
            let tag_mapper = TagMapper::new(tag_key);
            match tag_mapper.to_tag() {
                Ok(tag) => search_conditions.push(SearchCondition {
                    search_type: SearchType::Tag,
                    value: tag,
                }),
                Err(_) => {
                    err_message_list.push("不正なタグIDです。".to_string());
                }
            }
        }
    }

    // 検索条件: 山名
    if let Some(mountain_name) = query_params.get("name") {
        search_conditions.push(SearchCondition {
            search_type: SearchType::Name,
            value: mountain_name.to_string(),
        });
    }

    // offset, limit 値チェック
    let mut offset_value = 0 as usize;
    if let Some(offset) = query_params.get("offset") {
        if let Ok(offset_temp) = offset.to_string().parse::<usize>() {
            offset_value = offset_temp;
        } else {
            err_message_list.push("offsetは0以上の整数を指定してください。".to_string());
        }
    }

    let mut limit_value: Option<usize> = None;
    if let Some(limit) = query_params.get("limit") {
        if let Ok(limit_temp) = limit.to_string().parse::<usize>() {
            if limit_temp > 0 {
                limit_value = Some(limit_temp);
            } else {
                err_message_list.push("limitは1以上の整数を指定してください。".to_string());
            }
        } else {
            err_message_list.push("limitは1以上の整数を指定してください。".to_string());
        }
    }

    // sort
    let mut sort_key = "id".to_string();
    if let Some(sort) = query_params.get("sort") {
        let mut is_invalid_sort_value = true;
        let chk_keys = ["id.asc", "id.desc", "elevation.asc", "elevation.desc", "name.asc", "name.desc"];
        for key in chk_keys {
            let s_key = sort.to_string();
            if s_key == key.to_string() {
                sort_key = s_key;
                is_invalid_sort_value = false;
            }
        }
        if is_invalid_sort_value {
            err_message_list.push("不正なソート指定です。".to_string());
        }
    }

    if !err_message_list.is_empty() {
        return Err(err_message_list);
    }

    // offset and limit
    let range_condition =  RangeCondition {
        offset: offset_value,
        limit: limit_value,
    };

    // 検索条件が存在しない場合、scanを実行する
    if search_conditions.is_empty() {
        return match services::get_all_mountains(&client, range_condition).await {
            Ok(searched_mountain_result) => {
                match serde_json::to_string_pretty(&searched_mountain_result.mountains) {
                    Ok(result) => Ok(SearchedResult {
                        mountains_json: result,
                        total: searched_mountain_result.total,
                        offset: searched_mountain_result.offset,
                        limit: searched_mountain_result.limit,
                    }),
                    Err(_) => Err(simple_err_message_list),
                }
            }
            Err(_) => Err(simple_err_message_list),
        };
    }

    match services::search_mountains(&client, search_conditions, range_condition).await {
        Ok(searched_mountain_result) => {
            match serde_json::to_string_pretty(&searched_mountain_result.mountains) {
                Ok(result) => {
                    Ok(SearchedResult {
                        mountains_json: result,
                        total: searched_mountain_result.total,
                        offset: searched_mountain_result.offset,
                        limit: searched_mountain_result.limit,
                    })
                },
                Err(_) => Err(simple_err_message_list),
            }
        },
        Err(_) => Err(simple_err_message_list),
    }
}
