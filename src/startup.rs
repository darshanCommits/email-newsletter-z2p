use crate::{
    configuration::{AppState, DatabaseConfiguration, get_configuration},
    routes::get_router,
    telemetry::{get_subscriber, init_subscriber},
};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::io::Result;
use tokio::net::TcpListener;
use uuid::Uuid;

/// Only for integration tests.
#[derive(Debug)]
pub struct TestApp {
    pub addr: String,
    pub db_pool: PgPool,
}

/// Creating a uuid named db through `PgConnection` and then doing the migrations through `PgPool`
pub async fn configure_database(settings: DatabaseConfiguration) -> PgPool {
    let mut connection = PgConnection::connect_with(&settings.without_db())
        .await
        .expect("Failed to connect to Postgres.");

    connection
        .execute(format!(r#"CREATE DATABASE "{}""#, settings.name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect_with(settings.with_db())
        .await
        .expect("Failed to connect to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

pub async fn spawn_app_testing() -> Result<TestApp> {
    let subscriber = get_subscriber("z2p".into(), "debug".into(), std::io::stdout)?;
    init_subscriber(subscriber)?;

    let mut settings = get_configuration().expect("Failed to read Configuration");
    settings.database.name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(settings.database).await;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let actual_address = listener.local_addr()?;

    let test_app = TestApp {
        addr: format!("http://{actual_address}"),
        db_pool: connection_pool.clone(),
    };

    let app_state = AppState {
        db_pool: connection_pool,
    };

    let router = get_router(app_state);

    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("Test server failed");
    });

    Ok(test_app)
}
