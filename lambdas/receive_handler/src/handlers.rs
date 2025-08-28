use anyhow::Context;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use lib::env_keys::QUEUE_URL;
use lib::service::slack_service::{EventChallengeRequest, MessageEventRequest};
use lib::service::CommonService;
use serde_json::{json, Value};

const REQUEST_TIMESTAMP_HEADER: &str = "X-Slack-Request-Timestamp";
const REQUEST_SIGNATURE_HEADER: &str = "X-Slack-Signature";

pub async fn webhook_received(
    State(service): State<CommonService>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    let (timestamp, received_signature) = match get_timestamp_signature(&headers) {
        Ok((t, s)) => (t, s),
        Err(error) => {
            println!("error getting timestamp and signature: {}", error);
            return build_error_response(&error.to_string());
        }
    };

    let Ok(body_string) = String::from_utf8(bytes.to_vec()) else {
        println!("error getting body as string.");
        return build_error_response("error getting body as string.");
    };

    let verification_result =
        service
            .slack
            .verify_signature(timestamp, &body_string, &received_signature);

    if verification_result.is_err() || verification_result.unwrap() == false {
        println!("Error verifying request.");
        return build_error_response("Error Verifying request.");
    }

    let value: Value = serde_json::from_slice(&bytes).unwrap();

    let challenge_request = serde_json::from_value::<EventChallengeRequest>(value.clone());
    if let Ok(challenge_request) = challenge_request {
        if !service
            .slack
            .verify_url_verification_request(&challenge_request)
        {
            return build_error_response("Error Verifying.");
        } else {
            let response_body = json!({
                "challenge": challenge_request.challenge
            });
            return build_success_response(&response_body);
        }
    }

    let message_request = match serde_json::from_value::<MessageEventRequest>(value) {
        Ok(request) => request,
        Err(error) => {
            println!("Error converting to Message request: {:?}", error);
            return build_success_response(&json!({}));
        }
    };

    if !service.slack.verify_message_request(&message_request) {
        println!("event is not a message event.");
        return build_success_response(&json!({}));
    }

    let Ok(queue_url) = std::env::var(QUEUE_URL) else {
        println!("SQS URL not availabe");
        return build_success_response(&json!({}));
    };

    match service.sqs.send(&queue_url, &message_request).await {
        Ok(_) => {}
        Err(error) => {
            println!("Error sending to sqs: {}", error);
        }
    }

    return build_success_response(&json!({}));
}

fn get_timestamp_signature(headers: &HeaderMap) -> anyhow::Result<(u64, String)> {
    let timestamp_string = headers
        .get(REQUEST_TIMESTAMP_HEADER)
        .and_then(|header| header.to_str().ok())
        .context("no timestamp string received.")?;
    let timestamp: u64 = timestamp_string.parse()?;

    let received_signature = headers
        .get(REQUEST_SIGNATURE_HEADER)
        .and_then(|header| header.to_str().ok())
        .context("No signature received.")?;

    return Ok((timestamp, received_signature.to_owned()));
}

fn build_error_response(message: &str) -> Response {
    let mut json_header = HeaderMap::new();
    json_header.insert(CONTENT_TYPE, "application/json".parse().unwrap());

    let mut response = Response::new(
        json!({
            "success": false,
            "message": message
        })
        .to_string(),
    );
    *response.status_mut() = StatusCode::BAD_REQUEST;
    return (json_header, response).into_response();
}

fn build_success_response(body: &Value) -> Response {
    let mut json_header = HeaderMap::new();
    json_header.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    let response = Response::new(body.to_string());
    return (json_header, response).into_response();
}
