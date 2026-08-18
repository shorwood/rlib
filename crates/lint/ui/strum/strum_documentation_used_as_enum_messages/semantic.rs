#![allow(
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value,
    dead_code,
    rlib::method_like_free_functions,
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

struct Presenter;

impl Presenter {
    fn notify(&self, _message: &str) {}
}

fn notify_report(presenter: &Presenter, failure: Failure) {
    presenter.notify(failure.get_documentation().unwrap());
}

fn showroom_inventory(_documentation: &str) {}

fn internal_report(failure: Failure) {
    showroom_inventory(failure.get_documentation().unwrap());
}

#[derive(strum::EnumMessage)]
enum UndocumentedFailure {
    Rejected,
}

fn empty_report(failure: UndocumentedFailure) {
    show_message(failure.get_documentation().unwrap());
}

struct LocalDocumentation;

impl LocalDocumentation {
    fn get_documentation(&self) -> Option<&'static str> {
        Some("local")
    }
}

fn local_report(value: &LocalDocumentation) {
    show_message(value.get_documentation().unwrap());
}

fn main() {}
