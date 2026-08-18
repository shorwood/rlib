#![feature(register_tool)]
#![allow(
    dead_code,
    rlib::leptos_boolean_component_props,
    rlib::leptos_effects_synchronizing_signals,
    rlib::leptos_implicit_default_component_props,
    rlib::leptos_manual_resource_refetch_signals,
    rlib::leptos_missing_view_section_comments,
    rlib::leptos_needlessly_cloned_signal_values,
    rlib::leptos_read_then_replace_signals,
    rlib::leptos_reactive_writes_during_view_construction,
    rlib::leptos_reactive_writes_in_resource_fetchers,
    rlib::leptos_unkeyed_reactive_collections,
    rlib::leptos_unsanitized_inner_html,
    rlib::leptos_writable_signal_component_props,
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
fn FallibleFrozen() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <span>{count.try_get().unwrap_or_default()}</span>
        <span>{*count.try_read().expect("live signal")}</span>
        <span>{count.try_with(|count| count * 2).unwrap_or_default()}</span>
    }
}

#[component]
fn Reactive() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <span>{count}</span>
        <span>{move || count.get() * 2}</span>
        <span>{move /* still an authored closure */ || count.get() * 3}</span>
        <button on:click=move |_| count.set(count.get() + 1)>"increment"</button>
    }
}

#[component]
fn ExplicitSnapshot() -> impl IntoView {
    let count = RwSignal::new(1);
    view! { <span>{count.try_get_untracked().unwrap_or_default()}</span> }
}

fn outside_view() {
    let count = RwSignal::new(1);
    let _snapshot = count.get();
}

#[allow(rlib::leptos_unreactive_signal_reads_in_views)]
#[component]
fn Suppressed() -> impl IntoView {
    let count = RwSignal::new(1);
    view! { <span>{count.get()}</span> }
}

fn main() {}
