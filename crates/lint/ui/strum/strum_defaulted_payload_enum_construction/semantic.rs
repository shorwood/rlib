#![allow(dead_code, unknown_lints)]

#[derive(strum::EnumIter)]
enum Job {
    Pending(String),
    Complete,
}

#[derive(strum::EnumString)]
enum ParsedJob {
    Pending(String),
    #[strum(default)]
    Unknown(String),
}

fn generated_id() -> String {
    String::from("generated")
}

#[derive(strum::EnumString)]
enum ExplicitParsedJob {
    #[strum(default_with = "generated_id")]
    Pending(String),
    Created {
        #[strum(default_with = "generated_id")]
        id: String,
    },
}

#[derive(strum::EnumIter)]
enum IteratedJob {
    #[strum(default_with = "generated_id")]
    Pending(String),
}

#[derive(strum::EnumIter)]
enum MarkerJob<T> {
    Pending(std::marker::PhantomData<T>),
}

fn main() {}
