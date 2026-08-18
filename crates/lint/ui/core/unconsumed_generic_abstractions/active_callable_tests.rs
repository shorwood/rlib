// compile-flags: --test

#![warn(rlib::unconsumed_generic_abstractions)]
#![allow(dead_code, rlib::method_like_free_functions)]

struct Production;

struct Substitute;

fn test_substitution<T>(value: T) -> T {
    value
}

fn test_forwarding<T>(value: T) -> T {
    value
}

fn active_generic_consumer<U>(value: U) -> U {
    let forwarded = test_forwarding(value);
    forwarded
}

fn production_calls() {
    let _ = test_substitution(Production);
    let _ = test_forwarding(Production);
}

#[cfg(test)]
fn test_calls() {
    let _ = test_substitution(Substitute);
}
