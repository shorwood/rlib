#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    leptos_boolean_component_props,
    leptos_implicit_default_component_props,
    leptos_manual_resource_refetch_signals,
    leptos_reactive_writes_during_view_construction,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn cloned_for_inspection() {
    let (names, _) = signal(vec![String::from("Ada")]);
    let _ = names.get().len();
    let _ = names.get().is_empty();
}

fn valid_reads() {
    let (count, _) = signal(1_u32);
    let (names, _) = signal(vec![String::from("Ada")]);
    let _ = count.get();
    let _ = names.get().into_iter().next();
    let _ = names.read().len();
    let _ = names.with(Vec::len);
}

#[allow(leptos_needlessly_cloned_signal_values)]
fn suppressed() {
    let (names, _) = signal(Vec::<String>::new());
    let _ = names.get().len();
}

fn main() {}
