// edition:2024

#![feature(register_tool)]
#![allow(dead_code, private_interfaces, unused_imports, unused_variables)]
#![register_tool(rlib_lint)]

struct Item<T>(T);
struct Unit;

impl Unit {
    fn duplicate(&self) {}
}

// Public API and attributes may carry contracts that an automatic move cannot preserve.
pub fn public(item: Unit) {}

#[inline]
fn attributed(item: Unit) {}

// A pattern has no single binding to rename to `self`.
fn destructured(Item(value): Item<u8>) {}

// Bounds need deliberate placement on either the impl or the method.
fn constrained<T: Clone>(item: Item<T>) {}

// An alias can hide the reference shape and its lifetime contract.
type UnitRef<'a> = &'a Unit;
fn alias_reference(item: UnitRef<'_>) {}

// Moving an imported function or creating a duplicate method needs an API decision.
use imported as imported_alias;
fn imported(item: Unit) {}

fn duplicate(item: &Unit) {
    item.duplicate();
}

fn main() {}
