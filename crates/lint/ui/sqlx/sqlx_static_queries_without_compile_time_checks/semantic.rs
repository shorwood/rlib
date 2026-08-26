#![allow(dead_code, unknown_lints)]

fn query() {
    let _query = sqlx::query::<sqlx::Sqlite>("SELECT 1");
}

fn main() {}
