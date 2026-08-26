#![allow(dead_code, unknown_lints)]

async fn insert(pool: &sqlx::SqlitePool, values: &[i64]) -> Result<(), sqlx::Error> {
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new("INSERT INTO numbers(value) ");
    builder.push_values(values, |mut row, value| {
        row.push_bind(value);
    });
    builder.build().execute(pool).await?;
    Ok(())
}

fn main() {}
