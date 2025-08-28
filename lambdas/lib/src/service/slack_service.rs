use anyhow::Result;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;

use crate::{
    env_keys::{BOT_OAUTH_TOKEN, SLACK_SIGNING_SECRET},
    service::bedrock_service::RetrievalResult,
};

pub const EVENT_CALLBACK_TYPE: &str = "event_callback";
pub const URL_VERIFICATION_TYPE: &str = "url_verification";
pub const APP_MENTION_EVENT_TYPE: &str = "app_mention";

const POST_MESSAGE_ENDPOINT: &str = "https://slack.com/api/chat.postMessage";
const VERSION_NUMBER: &str = "v0";

#[derive(Debug, Clone)]
pub struct SlackService {
    client: Client,
    headers: HeaderMap,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct EventChallengeRequest {
    pub challenge: String,
    pub token: String,
    pub r#type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MessageEventRequest {
    pub token: String,
    pub api_app_id: String,
    pub r#type: String, // event_callback
    pub event_id: String,
    pub event_time: u64,
    pub event: AppMentionMessageEvent,
}

/// https://api.slack.com/events/app_mention
/// ```
///  {
///     "type": "app_mention",
///     "user": "U061F7AUR",
///     "text": "<@U0LAN0Z89> is it everything a river should be?",
///     "ts": "1515449522.000016",
///     "channel": "C123ABC456",
///     "event_ts": "1515449522000016"
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AppMentionMessageEvent {
    pub channel: String,
    pub r#type: String,   // app_mention
    pub event_ts: String, // thread_ts
    pub text: String,
    pub user: String,
}

impl SlackService {
    pub fn new() -> Self {
        let token: String = std::env::var(BOT_OAUTH_TOKEN).unwrap_or("".to_owned());
        let mut headers = HeaderMap::new();
        let bearer = format!("Bearer {}", token).to_string();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&bearer).unwrap_or(HeaderValue::from_static("")),
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json;charset=UTF-8"),
        );

        Self {
            client: Client::new(),
            headers,
        }
    }

    // https://api.slack.com/authentication/verifying-requests-from-slack
    pub fn verify_signature(
        &self,
        timestamp: u64,
        request_body: &str,
        received_signature: &str,
    ) -> Result<bool> {
        let signing_secret = std::env::var(SLACK_SIGNING_SECRET)?;

        let now = Utc::now().timestamp();
        // The request timestamp is more than five minutes from local time. ignore
        if (now - timestamp as i64).abs() > 60 * 5 {
            return Ok(false);
        }
        let sig_basestring = format!("{}:{}:{}", VERSION_NUMBER, timestamp, request_body);

        let mut mac = Hmac::<Sha256>::new_from_slice(signing_secret.as_bytes())?;
        mac.update(sig_basestring.as_bytes());
        let result = mac.finalize();
        let result_bytes = result.into_bytes();
        let hex_digest = hex::encode(&result_bytes); // Converts the byte array to a hex string
        let calculated_signature = format!("{}={}", VERSION_NUMBER, hex_digest);

        return Ok(calculated_signature == received_signature);
    }

    pub fn verify_url_verification_request(
        &self,
        event_challenge_request: &EventChallengeRequest,
    ) -> bool {
        return event_challenge_request.r#type == URL_VERIFICATION_TYPE;
    }

    pub fn verify_message_request(&self, message_request: &MessageEventRequest) -> bool {
        return message_request.r#type == EVENT_CALLBACK_TYPE
            && message_request.event.r#type == APP_MENTION_EVENT_TYPE;
    }

    pub async fn send_retrieve_result(
        &self,
        channel_id: &str,
        thread_ts: &str,
        user_id: &str,
        result: &RetrievalResult,
    ) -> Result<()> {
        let references: Vec<String> = result
            .reference_urls
            .iter()
            .enumerate()
            .map(|(index, url)| format!("{}: <{}>", index + 1, url))
            .collect();
        let reference_string = if references.is_empty() {
            ""
        } else {
            &format!("\n\nRelated URLs: \n{}", references.join("\n"))
        };

        let body = json!({
            "channel": channel_id,
            "thread_ts": thread_ts,
            "blocks": [
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("<@{}>\n{}{}", user_id, result.text, reference_string)
                    }
                }
            ]
        });

        let response = self
            .client
            .post(POST_MESSAGE_ENDPOINT)
            .headers(self.headers.clone())
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let body_string = response.text().await?;
        println!("response_body: {}", body_string);

        Ok(())
    }
}
