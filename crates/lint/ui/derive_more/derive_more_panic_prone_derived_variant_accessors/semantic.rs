#![allow(dead_code, unknown_lints)]

#[derive(derive_more::Unwrap)]
enum Response {
    Ok(String),
    Error(String),
}

fn unchecked(response: Response) -> String {
    response.unwrap_ok()
}

fn established() -> String {
    Response::Ok(String::new()).unwrap_ok()
}

fn main() {}
