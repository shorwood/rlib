#![allow(dead_code, unknown_lints)]

fn query() {
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT 1");
    let query = builder.build();
    drop(query);
    builder.push(" LIMIT 1");

    let mut reset = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT 1");
    let query = reset.build();
    drop(query);
    reset.reset();
    reset.push(" LIMIT 1");
}

fn main() {}
