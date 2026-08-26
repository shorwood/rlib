#![allow(dead_code, unknown_lints)]

fn query(fragment: &str) {
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT 1 ");
    builder.push(fragment);
    builder.push("LIMIT 1");
}

fn main() {}
