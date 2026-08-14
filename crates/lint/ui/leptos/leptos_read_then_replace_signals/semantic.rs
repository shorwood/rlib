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

fn fallible_untracked_replacement() {
    let count = RwSignal::new(0);
    let _ = count.try_set(count.try_get_untracked().unwrap_or_default() + 1);
}

fn borrowed_guard_replacement() {
    let count = RwSignal::new(0);
    (&count).set(*(&count).read() + 1);
}

struct State {
    count: RwSignal<i32>,
    other: RwSignal<i32>,
}

fn projected_signal_replacement() {
    let state = State {
        count: RwSignal::new(0),
        other: RwSignal::new(0),
    };
    state.count.set(state.count.with(|count| count + 1));
}

fn immediately_invoked_read() {
    let count = RwSignal::new(0);
    count.set((|| count.get() + 1)());
}

fn independent_replacement() {
    let count = RwSignal::new(0);
    count.set(42);
}

fn bounded_update() {
    let count = RwSignal::new(0);
    count.update(|count| *count += 1);
}

fn different_signal_is_independent() {
    let state = State {
        count: RwSignal::new(0),
        other: RwSignal::new(1),
    };
    state.count.set(state.other.get());
}

fn deferred_callback_is_independent() {
    let count = RwSignal::new(0);
    let callback = move || count.get();
    count.set(42);
    let _ = callback;
}

#[allow(leptos_read_then_replace_signals)]
fn suppressed() {
    let count = RwSignal::new(0);
    count.set(count.get() + 1);
}

fn main() {}
