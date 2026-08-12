#![feature(register_tool)]
#![allow(
    dead_code,
    leptos_boolean_component_props,
    leptos_effects_synchronizing_signals,
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

fn local_resource_write() {
    let status = RwSignal::new(String::new());
    let _resource = LocalResource::new(move || async move {
        status.set(String::from("loaded"));
        42
    });
}

fn resource_write() {
    let source = RwSignal::new(1);
    let selected = RwSignal::new(0);
    let _resource = Resource::new(
        move || source.get(),
        move |value| async move {
            selected.set(value);
            value
        },
    );
}

fn pure_resource() {
    let source = RwSignal::new(1);
    let _resource = Resource::new(move || source.get(), |value| async move { value * 2 });
}

fn nested_callback() {
    let status = RwSignal::new(false);
    let _resource = LocalResource::new(move || async move {
        let callback = move || status.set(true);
        let _ = callback;
    });
}

#[allow(leptos_reactive_writes_in_resource_fetchers)]
fn suppressed() {
    let status = RwSignal::new(false);
    let _resource = LocalResource::new(move || async move {
        status.set(true);
    });
}

fn main() {}
