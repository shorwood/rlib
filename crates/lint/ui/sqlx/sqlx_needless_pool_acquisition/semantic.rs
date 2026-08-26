#![allow(dead_code, unknown_lints)]

async fn query(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let mut connection = pool.acquire().await?;
    sqlx::query("DELETE FROM jobs")
        .execute(&mut *connection)
        .await?;
    Ok(())
}

fn main() {}
