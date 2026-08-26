#![allow(dead_code, unknown_lints)]

use sqlx::Row;

struct User {
    id: i64,
    name: String,
}

fn decode(row: &sqlx::sqlite::SqliteRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
    })
}

fn main() {}
