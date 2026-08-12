#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    leptos_boolean_component_props,
    leptos_implicit_default_component_props,
    leptos_manual_resource_refetch_signals,
    leptos_needlessly_cloned_signal_values,
    leptos_reactive_writes_during_view_construction,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn DynamicOwned(markup: String) -> impl IntoView {
    view! { <article inner_html=markup/> }
}

#[component]
fn DynamicBorrowed(markup: &'static str) -> impl IntoView {
    view! { <article inner_html=markup/> }
}

#[component]
fn StaticMarkup() -> impl IntoView {
    view! { <article inner_html="<strong>Reviewed</strong>"/> }
}

#[allow(leptos_unsanitized_inner_html)]
#[component]
fn TrustedBoundary(markup: String) -> impl IntoView {
    view! { <article inner_html=markup/> }
}

fn main() {}
