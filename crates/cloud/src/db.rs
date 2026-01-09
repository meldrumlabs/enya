//! Database connection and queries.

use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub mod models;
pub mod queries;

/// Create a database connection pool.
pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    Ok(pool)
}
