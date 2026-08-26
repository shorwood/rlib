#![allow(dead_code, unknown_lints)]

fn query(sql: String) {
    let _query = sqlx::query::<sqlx::Sqlite>(sqlx::AssertSqlSafe(sql));
}

fn literal() {
    let _query = sqlx::query::<sqlx::Sqlite>(sqlx::AssertSqlSafe("SELECT 1"));
}

fn main() {}
