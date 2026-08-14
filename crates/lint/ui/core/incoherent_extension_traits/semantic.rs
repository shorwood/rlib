// aux-build: foreign_types.rs
// edition:2024

#![warn(incoherent_extension_traits)]
#![allow(dead_code, unused_variables)]

extern crate foreign_types;

use foreign_types::{ExternalItem, ExternalTarget, ExternalType};

trait BroadExt {
    fn inspect_item(&self, item: &ExternalItem);
    fn inspect_type(&self, ty: &ExternalType);
}

impl BroadExt for ExternalTarget {
    fn inspect_item(&self, item: &ExternalItem) {}
    fn inspect_type(&self, ty: &ExternalType) {}
}

trait LargeExt {
    fn first(&self);
    fn second(&self);
    fn third(&self);
    fn fourth(&self);
    fn fifth(&self);
    fn sixth(&self);
    fn seventh(&self);
    fn eighth(&self);
    fn ninth(&self);
}

impl LargeExt for ExternalTarget {
    fn first(&self) {}
    fn second(&self) {}
    fn third(&self) {}
    fn fourth(&self) {}
    fn fifth(&self) {}
    fn sixth(&self) {}
    fn seventh(&self) {}
    fn eighth(&self) {}
    fn ninth(&self) {}
}

trait FocusedExt {
    fn inspect(&self, item: &ExternalItem);
    fn label(&self, label: &str);
    fn owned_label(&self, label: String);
    fn transform<T>(&self, transform: T);
}

impl FocusedExt for ExternalTarget {
    fn inspect(&self, item: &ExternalItem) {}
    fn label(&self, label: &str) {}
    fn owned_label(&self, label: String) {}
    fn transform<T>(&self, transform: T) {}
}

trait BroadBlanketExt {
    fn inspect_item(&self, item: &ExternalItem);
    fn inspect_type(&self, ty: &ExternalType);
}

impl<T> BroadBlanketExt for T {
    fn inspect_item(&self, item: &ExternalItem) {}
    fn inspect_type(&self, ty: &ExternalType) {}
}

fn main() {}
