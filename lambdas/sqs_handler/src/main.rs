use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{
    service_fn,
    tracing::{self},
    Error, LambdaEvent,
};
use lib::{
    env_keys::QUEUE_ARN,
    service::{slack_service::MessageEventRequest, CommonService},
};
use regex::Regex;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_08_07()).await;

    let service = CommonService::new(&config);
    let service_function = service_fn(|event| async { sqs_handler(event, &service).await });
    lambda_runtime::run(service_function).await?;

    Ok(())
}

async fn sqs_handler(
    event: LambdaEvent<SqsEvent>,
    service: &CommonService,
) -> Result<Value, Error> {
    println!("{:?}", event.payload);
    match process_event(event.payload, service).await {
        Ok(_) => {
            println!("finish processing sqs event with success!")
        }
        Err(error) => {
            println!("Error processing sqs event: {:?}", error)
        }
    }
    return Ok(json!({}));
}

async fn process_event(event: SqsEvent, service: &CommonService) -> anyhow::Result<()> {
    let queue_arn = std::env::var(QUEUE_ARN)?;

    for record in event.records.into_iter() {
        if record.event_source_arn.is_some() && record.event_source_arn.unwrap() != queue_arn {
            println!("wrong event source ");
            continue;
        }

        let Some(message_string) = record.body else {
            continue;
        };

        let message_request = match serde_json::from_str::<MessageEventRequest>(&message_string) {
            Ok(request) => request,
            Err(error) => {
                println!("error parsing message: {:?}", error);
                continue;
            }
        };

        let event = message_request.event;
        let input = remove_user_id(&event.text);

        if input.is_empty() {
            continue;
        }
        let result = match service.bedrock.retrieve(&input).await {
            Ok(r) => r,
            Err(error) => {
                println!("error retrieving: {}", error);
                continue;
            }
        };

        match service
            .slack
            .send_retrieve_result(&event.channel, &event.event_ts, &event.user, &result)
            .await
        {
            Ok(_) => {}
            Err(error) => {
                println!("error sending message to slack: {}", error);
                continue;
            }
        };
    }

    Ok(())
}

fn remove_user_id(text: &str) -> String {
    let Ok(re) = Regex::new(r"(<@(.*?)>)*") else {
        return text.to_owned();
    };

    let new = re.replace_all(text, "");
    return new.to_string();
}
