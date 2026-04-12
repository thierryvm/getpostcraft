use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

pub mod history;
pub mod settings_db;

pub async fn init_pool() -> Result<SqlitePool, String> {
    let db_path = dirs::data_dir()
        .ok_or("Cannot resolve data dir")?
        .join("getpostcraft")
        .join("app.db");

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.to_string_lossy()))
            .map_err(|e| e.to_string())?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| e.to_string())?;

    // Run migrations
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(pool)
}
