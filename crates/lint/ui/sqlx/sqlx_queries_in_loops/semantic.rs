#![allow(dead_code, unknown_lints)]

async fn delete(pool: &sqlx::SqlitePool, ids: &[i64]) -> Result<(), sqlx::Error> {
    for id in ids {
        sqlx::query("DELETE FROM jobs WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn main() {}
