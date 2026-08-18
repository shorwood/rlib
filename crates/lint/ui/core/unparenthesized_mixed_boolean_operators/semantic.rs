#![warn(rlib::unparenthesized_mixed_boolean_operators)]
#![allow(dead_code, rlib::misordered_module_declarations)]

fn ambiguous(first: bool, second: bool, alternative: bool) -> bool {
    first && second || alternative
}

fn ambiguous_right(first: bool, second: bool, alternative: bool) -> bool {
    first || second && alternative
}

fn ambiguous_chain(first: bool, second: bool, third: bool, fourth: bool) -> bool {
    first || second && third || fourth
}

fn explicit(first: bool, second: bool, alternative: bool) -> bool {
    (first && second) || alternative
}

fn governed(first: bool, second: bool, alternative: bool) -> bool {
    first && (second || alternative)
}

fn explicit_right(first: bool, second: bool, alternative: bool) -> bool {
    first || (second && alternative)
}

fn block_grouped(first: bool, second: bool, alternative: bool) -> bool {
    first && { second || alternative }
}

fn uniform(first: bool, second: bool, third: bool) -> bool {
    first && second && third
}

macro_rules! generated_condition {
    ($first:expr, $second:expr, $third:expr) => {
        $first && $second || $third
    };
}

fn main() {
    let _ = generated_condition!(true, true, false);
}
