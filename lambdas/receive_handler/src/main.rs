pub mod handlers;
use axum::routing::post;
use axum::Router;
use handlers::webhook_received;
use lambda_http::{run, tracing, Error};
use lib::service::CommonService;
use std::env::set_var;

#[tokio::main]
async fn main() -> Result<(), Error> {
    set_var("AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH", "true");

    tracing::init_default_subscriber();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_08_07()).await;
    let service = CommonService::new(&config);

    let app = Router::new()
        .route("/", post(post(webhook_received)))
        .with_state(service);

    run(app).await
}
