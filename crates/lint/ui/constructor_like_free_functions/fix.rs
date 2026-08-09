// run-rustfix

#![warn(constructor_like_free_functions)]
#![allow(dead_code)]

struct Token(String);

fn token_from_raw(raw: String) -> Token {
    Token(raw)
}

fn use_token() {
    let _ = token_from_raw(String::new());
}

fn main() {
    let constructor = token_from_raw;
    let _ = constructor(String::new());
}
