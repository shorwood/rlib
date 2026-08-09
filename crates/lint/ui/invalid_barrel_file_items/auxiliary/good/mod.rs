//! A valid barrel may document why its child modules exist.

pub(crate) mod private_child;
pub mod public_child;

#[path = "renamed_child.rs"]
pub(super) mod renamed;
