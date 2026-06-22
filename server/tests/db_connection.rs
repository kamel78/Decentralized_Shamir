use sea_orm::Database;
use dotenvy::dotenv;
use std::env;

#[tokio::test]
async fn test_database_connection() {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("Missing DATABASE_URL in environment");

    match Database::connect(&database_url).await {
        Ok(_) => println!(" Successfully connected to the database."),
        Err(err) => panic!(" Failed to connect to the database: {:?}", err),
    }
}
