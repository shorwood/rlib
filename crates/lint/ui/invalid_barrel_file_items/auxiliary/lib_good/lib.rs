//! A library root may document the crate while remaining a barrel.

#![allow(dead_code)]

#[path = "lib_child.rs"]
pub mod child;
