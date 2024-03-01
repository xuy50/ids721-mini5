use serde::{Deserialize, Serialize};
use rusoto_core::Region;
use rusoto_dynamodb::{AttributeValue, DynamoDb, DynamoDbClient, ScanInput};
use lambda_http::{run, service_fn, Body, Request, Response, RequestExt}; // Included RequestExt here
use std::error::Error;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

#[derive(Debug, Deserialize, Serialize)]
struct Record {
    #[serde(rename = "date")]
    date: String,
    #[serde(rename = "product")]
    product: String,
    #[serde(rename = "price")]
    price: f64,
    #[serde(rename = "quantity")]
    quantity: i32,
}

async fn query_dynamodb_data(event: Request) -> Result<Response<Body>, Box<dyn Error>> {
    let query_params = event.query_string_parameters_ref();

    let low_price = match query_params.and_then(|params| params.first("low")) {
        Some(value) => value.parse::<f64>().map_err(|_| "Invalid 'low' parameter. Must be a floating-point number.")?,
        None => 0.0,
    };

    let high_price = match query_params.and_then(|params| params.first("high")) {
        Some(value) => value.parse::<f64>().map_err(|_| "Invalid 'high' parameter. Must be a floating-point number.")?,
        None => 10000.0,
    };

    let dynamodb_client = DynamoDbClient::new(Region::UsEast2);

    let expression_attribute_values = Some([
        (
            ":low_price".to_string(),
            AttributeValue {
                n: Some(low_price.to_string()),
                ..Default::default()
            },
        ),
        (
            ":high_price".to_string(),
            AttributeValue {
                n: Some(high_price.to_string()),
                ..Default::default()
            },
        ),
    ].iter().cloned().collect());

    let scan_input = ScanInput {
        table_name: "721_mini5_data".to_string(),
        filter_expression: Some("price BETWEEN :low_price AND :high_price".to_string()),
        expression_attribute_values,
        ..Default::default()
    };

    let result = dynamodb_client.scan(scan_input).await?;
    let items = result.items.ok_or_else(|| "No items found in the scan")?;

    let records: Vec<Record> = items.into_iter().map(|item| {
        let date = item.get("date")
            .and_then(|v| v.s.as_ref().map(|s| s.to_string()))
            .unwrap_or_default();
        let product = item.get("product")
            .and_then(|v| v.s.as_ref().map(|s| s.to_string()))
            .unwrap_or_default();
        let price = item.get("price")
            .and_then(|v| v.n.as_ref())
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or_default();
        let quantity = item.get("quantity")
            .and_then(|v| v.n.as_ref())
            .and_then(|n| n.parse::<i32>().ok())
            .unwrap_or_default();

        Record { date, product, price, quantity }
    }).collect();

    let response_body = serde_json::to_string(&records)?;
    Ok(Response::new(response_body.into()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_target(false)
        .without_time()
        .init();

    run(service_fn(query_dynamodb_data)).await
}
