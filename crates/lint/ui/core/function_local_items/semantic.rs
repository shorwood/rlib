#![allow(dead_code, unused_imports)]
#![warn(function_local_items)]

struct ModuleType;

impl ModuleType {}

fn contains_local_items() {
    use std::fmt;

    const CAPACITY: usize = 4;

    static ENABLED: bool = true;

    type Identifier = usize;

    struct Local;

    enum State {
        Ready,
    }

    union Value {
        integer: u8,
    }

    trait Operation {}

    impl Operation for Local {}

    fn helper() {}

    mod support {}

    macro_rules! local_macro {
        () => {};
    }

    let _ = (fmt::Error, CAPACITY, ENABLED, Identifier::default());
    helper();
    local_macro!();
}

fn closure_contains_item() {
    let closure = || {
        struct ClosureLocal;
    };
    closure();
}

macro_rules! generated_local_item {
    () => {
        struct GeneratedLocal;
    };
}

fn generated_is_ignored() {
    generated_local_item!();
}

fn main() {
    contains_local_items();
    closure_contains_item();
    generated_is_ignored();
}
