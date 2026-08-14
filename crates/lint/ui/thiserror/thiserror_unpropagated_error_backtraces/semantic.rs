#![allow(dead_code, unknown_lints)]
#![feature(error_generic_member_access)]

use std::backtrace::Backtrace;

#[derive(Debug, thiserror::Error)]
#[error("inner")]
struct Inner {
    backtrace: Backtrace,
}

#[derive(Debug, thiserror::Error)]
#[error("duplicate")]
struct Duplicate {
    #[source]
    source: Inner,
    backtrace: Backtrace,
}

#[derive(Debug, thiserror::Error)]
#[error("missing")]
struct Missing {
    #[source]
    source: Inner,
}

#[derive(Debug, thiserror::Error)]
#[error("forwarded")]
struct Forwarded {
    #[source]
    #[backtrace]
    source: Inner,
}

#[derive(Debug, thiserror::Error)]
enum EnumWrapper {
    #[error("duplicate enum")]
    Duplicate {
        #[source]
        source: Inner,
        backtrace: Backtrace,
    },
    #[error("missing enum")]
    Missing(#[source] Inner),
    #[error("forwarded enum")]
    Forwarded(
        #[source]
        #[backtrace]
        Inner,
    ),
}

#[derive(Debug, thiserror::Error)]
enum SplitWrapper {
    #[error("source variant")]
    Source {
        #[source]
        source: Inner,
    },
    #[error("capture variant")]
    Capture { backtrace: Backtrace },
}

fn main() {}
