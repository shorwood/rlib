#![feature(register_tool)]
#![allow(
    dead_code,
    leptos_boolean_component_props,
    leptos_effects_synchronizing_signals,
    leptos_implicit_default_component_props,
    leptos_manual_resource_refetch_signals,
    leptos_needlessly_cloned_signal_values,
    leptos_reactive_writes_during_view_construction,
    leptos_reactive_writes_in_resource_fetchers,
    leptos_unkeyed_reactive_collections,
    leptos_unreactive_signal_reads_in_views,
    leptos_unsanitized_inner_html,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn scalar_replacement() {
    let count = RwSignal::new(0);
    count.set(count.get() + 1);
}

fn collection_replacement() {
    let items = RwSignal::new(vec![1]);
    items.set({
        let mut next = items.get();
        next.push(2);
        next
    });
}

fn independent_replacement() {
    let count = RwSignal::new(0);
    count.set(42);
}

fn bounded_update() {
    let count = RwSignal::new(0);
    count.update(|count| *count += 1);
}

fn nested_callback_is_independent() {
    let count = RwSignal::new(0);
    let callback = move || count.get();
    count.set(callback());
}

#[allow(leptos_read_then_replace_signals)]
fn suppressed() {
    let count = RwSignal::new(0);
    count.set(count.get() + 1);
}

fn main() {}
