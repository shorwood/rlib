#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    rlib::leptos_boolean_component_props,
    rlib::leptos_implicit_default_component_props,
    rlib::leptos_manual_resource_refetch_signals,
    rlib::leptos_reactive_writes_during_view_construction,
    rlib::leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[derive(Clone)]
struct Consuming(Vec<u8>);

impl Consuming {
    fn len(self) -> usize {
        self.0.len()
    }
}

#[derive(Clone)]
struct Mutating(Vec<u8>);

impl Mutating {
    fn is_empty(&mut self) -> bool {
        self.0.clear();
        self.0.is_empty()
    }
}

#[derive(Clone)]
struct Borrowing(Vec<u8>);

impl Borrowing {
    fn len(&self) -> usize {
        self.0.len()
    }
}

fn cloned_for_inspection() {
    let (names, _) = signal(vec![String::from("Ada")]);
    let _ = names.get().len();
    let _ = names.get().is_empty();
    let borrowing = RwSignal::new(Borrowing(vec![1]));
    let _ = borrowing.get().len();
}

fn valid_reads() {
    let (count, _) = signal(1_u32);
    let (names, _) = signal(vec![String::from("Ada")]);
    let _ = count.get();
    let _ = names.get().into_iter().next();
    let _ = names.read().len();
    let _ = names.with(Vec::len);
    let consuming = RwSignal::new(Consuming(vec![1]));
    let mutating = RwSignal::new(Mutating(vec![1]));
    let _ = consuming.get().len();
    let _ = mutating.get().is_empty();
}

#[allow(rlib::leptos_needlessly_cloned_signal_values)]
fn suppressed() {
    let (names, _) = signal(Vec::<String>::new());
    let _ = names.get().len();
}

fn main() {}
