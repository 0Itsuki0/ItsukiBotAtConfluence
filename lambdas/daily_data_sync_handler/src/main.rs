use aws_lambda_events::eventbridge::EventBridgeEvent;
use lambda_runtime::{
    service_fn,
    tracing::{self},
    Error, LambdaEvent,
};
use lib::service::CommonService;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_08_07()).await;
    let service = CommonService::new(&config);
    let service_function = service_fn(|event| async { eventbridge_handler(event, &service).await });
    lambda_runtime::run(service_function).await?;

    Ok(())
}

async fn eventbridge_handler(
    event: LambdaEvent<EventBridgeEvent>,
    service: &CommonService,
) -> Result<Value, Error> {
    println!("{:?}", event.payload);
    match process_event(service).await {
        Ok(_) => {
            println!("finish processing event with success!")
        }
        Err(error) => {
            println!("Error processing event: {:?}", error)
        }
    }
    return Ok(json!({}));
}

async fn process_event(service: &CommonService) -> anyhow::Result<()> {
    service.bedrock.start_data_sync().await?;
    Ok(())
}
