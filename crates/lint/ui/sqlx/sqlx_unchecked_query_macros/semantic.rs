#![allow(dead_code, unknown_lints)]

fn query() {
    let _query = sqlx::query_unchecked!("SELECT 1 AS value");
}

fn main() {}
