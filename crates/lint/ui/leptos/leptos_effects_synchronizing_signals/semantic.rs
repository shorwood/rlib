#![feature(register_tool)]
#![allow(
    dead_code,
    leptos_boolean_component_props,
    leptos_implicit_default_component_props,
    leptos_manual_resource_refetch_signals,
    leptos_needlessly_cloned_signal_values,
    leptos_reactive_writes_during_view_construction,
    leptos_unkeyed_reactive_collections,
    leptos_unsanitized_inner_html,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn synchronized() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| {
        target.set(source.get() * 2);
    });
}

fn through_local() {
    let source = RwSignal::new(vec![1, 2]);
    let target = RwSignal::new(Vec::<i32>::new());
    Effect::new(move |_| {
        let selected = source
            .get()
            .into_iter()
            .filter(|value| value % 2 == 0)
            .collect();
        target.set(selected);
    });
}

fn watched() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::watch(
        move || source.get(),
        move |value, _, _| target.set(*value),
        false,
    );
}

fn external_effect() {
    let source = RwSignal::new(1);
    Effect::new(move |_| {
        println!("{}", source.get());
    });
}

fn event_callback() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| {
        let callback = move || target.set(source.get());
        let _ = callback;
    });
}

#[allow(leptos_effects_synchronizing_signals)]
fn suppressed() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| target.set(source.get()));
}

fn main() {}
