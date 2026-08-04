//! A valid barrel may document why its child modules exist.

mod private_child;
pub mod public_child;

#[path = "renamed_child.rs"]
mod renamed;

pub use public_child::PublicChild;
pub(crate) use private_child::CrateChild;
pub(super) use renamed::ParentChild;
pub(in crate) use private_child::AncestorChild;
