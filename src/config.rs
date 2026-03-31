use aws_config::BehaviorVersion;
use dynamodb::DynamoDbRepo;

pub fn table_name() -> String {
    std::env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string())
}

pub async fn create_repo() -> DynamoDbRepo {
    let table_name = table_name();
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);
    DynamoDbRepo::new(client, table_name)
}
