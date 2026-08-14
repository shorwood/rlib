#![allow(dead_code, unknown_lints)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

#[derive(Debug, thiserror::Error)]
#[error("local failure: {context}")]
pub struct LocalError {
    context: Rc<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("shared failure: {context}")]
pub struct SharedError {
    context: Arc<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("nested local failure")]
pub struct NestedLocalError {
    context: Option<Rc<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("movable failure")]
pub struct MovableError {
    context: RefCell<String>,
}

pub struct Loader;

impl Loader {
    pub fn nested_error_sender() -> Sender<NestedLocalError> {
        channel().0
    }
}

pub fn error_sender() -> Sender<LocalError> {
    channel().0
}

pub fn shared_error_sender() -> Sender<SharedError> {
    channel().0
}

pub fn movable_error_sender() -> Sender<MovableError> {
    channel().0
}

mod internal {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("private local failure")]
    pub struct PrivateError {
        context: Rc<String>,
    }

    pub fn private_sender() -> Sender<PrivateError> {
        channel().0
    }
}

pub mod mpsc {
    pub struct Sender<T>(pub T);
}

pub fn lookalike_sender(error: LocalError) -> mpsc::Sender<LocalError> {
    mpsc::Sender(error)
}

fn main() {}
