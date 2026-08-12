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
    leptos_unsanitized_inner_html,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn Frozen() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <span>{count.get()}</span>
        <div class=count.get().to_string()>"value"</div>
    }
}

#[component]
fn Reactive() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <span>{count}</span>
        <span>{move || count.get() * 2}</span>
        <button on:click=move |_| count.set(count.get() + 1)>"increment"</button>
    }
}

fn outside_view() {
    let count = RwSignal::new(1);
    let _snapshot = count.get();
}

#[allow(leptos_unreactive_signal_reads_in_views)]
#[component]
fn Suppressed() -> impl IntoView {
    let count = RwSignal::new(1);
    view! { <span>{count.get()}</span> }
}

fn main() {}
