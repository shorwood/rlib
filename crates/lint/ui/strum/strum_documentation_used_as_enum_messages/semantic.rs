#![allow(
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value,
    dead_code,
    unknown_lints
)]

use strum::EnumMessage;

#[derive(strum::EnumMessage)]
enum Failure {
    /// The request could not be completed.
    Rejected,
}

fn show_message(_message: &str) {}

fn report(failure: Failure) {
    show_message(failure.get_documentation().unwrap());
}

fn main() {}
