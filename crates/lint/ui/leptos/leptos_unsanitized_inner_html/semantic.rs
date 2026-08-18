#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    rlib::leptos_boolean_component_props,
    rlib::leptos_implicit_default_component_props,
    rlib::leptos_manual_resource_refetch_signals,
    rlib::leptos_needlessly_cloned_signal_values,
    rlib::leptos_reactive_writes_during_view_construction,
    rlib::leptos_writable_signal_component_props,
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
fn DynamicReactive(markup: String) -> impl IntoView {
    let markup = RwSignal::new(markup);
    view! { <article inner_html=move || markup.get()/> }
}

#[component]
fn StaticMarkup() -> impl IntoView {
    view! {
        <article inner_html="<strong>Reviewed</strong>"/>
        <article inner_html=move || "<small>Reviewed</small>".to_owned()/>
    }
}

#[allow(rlib::leptos_unsanitized_inner_html)]
#[component]
fn TrustedBoundary(markup: String) -> impl IntoView {
    view! { <article inner_html=markup/> }
}

fn main() {}
