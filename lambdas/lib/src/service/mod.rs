pub mod bedrock_service;
pub mod slack_service;
pub mod sqs_service;

use aws_config::SdkConfig;

#[derive(Debug, Clone)]
pub struct CommonService {
    pub bedrock: bedrock_service::BedrockService,
    pub sqs: sqs_service::SQSService,
    pub slack: slack_service::SlackService,
}

impl CommonService {
    pub fn new(config: &SdkConfig) -> Self {
        let bedrock_runtime_client = aws_sdk_bedrockagentruntime::Client::new(&config);
        let bedrock_client = aws_sdk_bedrockagent::Client::new(&config);
        let sqs_client = aws_sdk_sqs::Client::new(&config);

        let line_client = slack_service::SlackService::new();

        Self {
            bedrock: bedrock_service::BedrockService::new(&bedrock_runtime_client, &bedrock_client),
            sqs: sqs_service::SQSService::new(&sqs_client),
            slack: line_client,
        }
    }
}
