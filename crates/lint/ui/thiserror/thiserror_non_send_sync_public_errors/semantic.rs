#![allow(dead_code, unknown_lints)]

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

pub fn error_sender() -> Sender<LocalError> {
    channel().0
}

pub fn shared_error_sender() -> Sender<SharedError> {
    channel().0
}

fn main() {}
