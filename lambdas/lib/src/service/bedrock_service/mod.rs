use anyhow::{bail, Result};
use aws_sdk_bedrockagent::{
    operation::list_data_sources::{ListDataSourcesError, ListDataSourcesOutput},
    types::DataSourceSummary,
};
use aws_sdk_bedrockagentruntime::types::{
    KnowledgeBaseRetrieveAndGenerateConfiguration, RetrievalResultLocationType,
    RetrieveAndGenerateConfiguration, RetrieveAndGenerateInput,
};
use serde::{Deserialize, Serialize};
use std::env;

use crate::env_keys::{CHAT_MODEL_ID, KNOWLEDGE_BASE_ID};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RetrievalResult {
    pub text: String,
    pub reference_urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BedrockService {
    runtime_client: aws_sdk_bedrockagentruntime::Client,
    client: aws_sdk_bedrockagent::Client,
}

impl BedrockService {
    pub fn new(
        runtime_client: &aws_sdk_bedrockagentruntime::Client,
        client: &aws_sdk_bedrockagent::Client,
    ) -> Self {
        Self {
            runtime_client: runtime_client.to_owned(),
            client: client.to_owned(),
        }
    }

    pub async fn start_data_sync(&self) -> Result<()> {
        let knowledge_base_id = env::var(KNOWLEDGE_BASE_ID)?;
        let datasource_stream = self
            .client
            .list_data_sources()
            .knowledge_base_id(&knowledge_base_id)
            .into_paginator()
            .send();
        let results = datasource_stream
            .collect::<Vec<
                Result<
                    ListDataSourcesOutput,
                    aws_sdk_bedrockagent::error::SdkError<ListDataSourcesError, _>,
                >,
            >>()
            .await;

        let results: Result<Vec<ListDataSourcesOutput>, _> =
            results.into_iter().map(|r| r).collect();

        let summaries: Vec<DataSourceSummary> = match results {
            Ok(r) => r
                .iter()
                .flat_map(|r| r.data_source_summaries().to_vec())
                .collect(),
            Err(error) => {
                println!("Error getting datasource summary: {}, ", error);
                bail!(error)
            }
        };
        let datasource_ids: Vec<String> = summaries
            .iter()
            .map(|s| s.data_source_id().to_owned())
            .collect();

        println!("syncing data soucres: {:?}", datasource_ids);

        for id in datasource_ids {
            let result = self
                .client
                .start_ingestion_job()
                .knowledge_base_id(&knowledge_base_id)
                .data_source_id(id)
                .send()
                .await;
            if let Err(error) = result {
                println!("Error starting data sync: {}", error)
            }
        }

        Ok(())
    }

    pub async fn retrieve(&self, input_query: &str) -> Result<RetrievalResult> {
        let model_arn = env::var(CHAT_MODEL_ID)?;
        let knowledge_base_id = env::var(KNOWLEDGE_BASE_ID)?;

        let input = RetrieveAndGenerateInput::builder()
            .text(input_query)
            .build()?;

        let knowbase_configuration = KnowledgeBaseRetrieveAndGenerateConfiguration::builder()
            .knowledge_base_id(knowledge_base_id)
            .model_arn(model_arn)
            .build()?;

        let configuration = RetrieveAndGenerateConfiguration::builder()
            .knowledge_base_configuration(knowbase_configuration)
            .r#type(aws_sdk_bedrockagentruntime::types::RetrieveAndGenerateType::KnowledgeBase)
            .build()?;

        let response = self
            .runtime_client
            .retrieve_and_generate()
            .input(input)
            .retrieve_and_generate_configuration(configuration)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(error) => {
                println!("error getting response: {}", error);
                bail!(error)
            }
        };

        let Some(output) = response.output() else {
            bail!("Fail to generate an output for the input.")
        };

        let citations = response.citations();

        let reference_urls: Vec<String> = citations
            .iter()
            .flat_map(|c| c.retrieved_references().to_owned())
            .filter(|r| {
                r.location().is_some()
                    && r.location.clone().unwrap().r#type()
                        == &RetrievalResultLocationType::Confluence
                    && r.location().unwrap().confluence_location().is_some()
                    && r.location()
                        .unwrap()
                        .confluence_location()
                        .unwrap()
                        .url()
                        .is_some()
            })
            .map(|r| {
                r.location()
                    .unwrap()
                    .confluence_location()
                    .unwrap()
                    .url()
                    .unwrap()
                    .to_owned()
            })
            .collect();

        Ok(RetrievalResult {
            text: output.text().to_owned(),
            reference_urls,
        })
    }
}
