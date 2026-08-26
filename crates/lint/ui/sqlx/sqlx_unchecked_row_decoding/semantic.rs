#![allow(dead_code, unknown_lints)]

use sqlx::Row;

fn decode(row: &sqlx::sqlite::SqliteRow) {
    let _id: i64 = row.get_unchecked("id");
    let _name: Result<String, _> = row.try_get_unchecked("name");
    let _checked: Result<i64, _> = row.try_get("id");
}

fn main() {}
