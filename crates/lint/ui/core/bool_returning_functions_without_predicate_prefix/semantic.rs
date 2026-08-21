// aux-build: boolean_result_external_macro.rs

#![feature(register_tool)]
#![allow(dead_code, non_snake_case)]
#![warn(rlib::bool_returning_functions_without_predicate_prefix)]
#![register_tool(rlib_lint)]

extern crate boolean_result_external_macro;

use boolean_result_external_macro::external_function;
use std::future::{Future, ready as future};

type Flag = bool;
type Maybe<T> = Option<T>;
type Fallible<T> = Result<T, ()>;

// Direct, aliased, recursively wrapped, and async boolean results are checked.
fn ready() -> bool {
    true
}

fn visible() -> Flag {
    true
}

fn maybe_ready() -> Maybe<Flag> {
    Some(true)
}

fn fallible_ready() -> Fallible<Maybe<bool>> {
    Ok(Some(true))
}

async fn load_ready() -> Result<bool, ()> {
    Ok(true)
}

// Property prefixes and the standard callable query vocabulary are accepted.
fn is_ready() -> bool {
    true
}

fn has_items() -> bool {
    true
}

fn should_retry() -> bool {
    true
}

fn all() -> bool {
    true
}

fn any_ready() -> bool {
    true
}

fn contains() -> bool {
    true
}

fn ends_with_suffix() -> bool {
    true
}

fn exists() -> bool {
    true
}

fn starts_with_prefix() -> bool {
    true
}

fn try_exists() -> bool {
    true
}

fn r#is_raw_ready() -> bool {
    true
}

// Empty and malformed predicate phrases remain invalid.
fn is_() -> bool {
    true
}

fn should__retry() -> bool {
    true
}

fn contains_() -> bool {
    true
}

fn contains__item() -> bool {
    true
}

trait Probe {
    fn ready(&self) -> bool;

    fn visible(&self) -> Option<bool> {
        Some(true)
    }

    async fn load(&self) -> Result<bool, ()>;
}

struct Checker;

impl Checker {
    fn ready(&self) -> bool {
        true
    }

    fn construct() -> bool {
        true
    }

    fn is_active(&self) -> bool {
        true
    }
}

// Trait implementation methods are skipped so each contract is diagnosed once.
impl Probe for Checker {
    fn ready(&self) -> bool {
        true
    }

    async fn load(&self) -> Result<bool, ()> {
        Ok(true)
    }
}

// Only bool in the standard success path is classified as a boolean result.
struct Wrapper<T>(T);

fn wrapped() -> Wrapper<bool> {
    Wrapper(true)
}

fn error_flag() -> Result<u8, bool> {
    Ok(1)
}

fn flags() -> Vec<bool> {
    vec![true]
}

fn future_ready() -> impl Future<Output = bool> {
    future(true)
}

extern "C" fn exported_ready() -> bool {
    true
}

macro_rules! local_function {
    () => {
        fn enabled() -> bool {
            true
        }
    };
}

local_function!();
external_function!();

fn main() {
    let _predicate = || -> bool { true };
}
