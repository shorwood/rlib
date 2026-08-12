// run-rustfix

#![warn(constructor_like_free_functions)]
#![allow(dead_code)]

struct Token(String);

fn token_from_raw_unchecked(raw: String) -> Token {
    Token(raw)
}

fn use_token() {
    let _ = token_from_raw_unchecked(String::new());
}

fn main() {
    let constructor = token_from_raw_unchecked;
    let _ = constructor(String::new());
}
