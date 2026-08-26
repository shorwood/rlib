#![allow(dead_code, unknown_lints)]

async fn implicit(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let _transaction = pool.begin().await?;
    Ok(())
}

async fn explicit(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let transaction = pool.begin().await?;
    transaction.rollback().await?;
    Ok(())
}

fn main() {}
