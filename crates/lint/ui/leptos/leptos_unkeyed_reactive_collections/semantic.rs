#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    leptos_boolean_component_props,
    leptos_implicit_default_component_props,
    leptos_manual_resource_refetch_signals,
    leptos_needlessly_cloned_signal_values,
    leptos_reactive_writes_during_view_construction,
    leptos_unsanitized_inner_html,
    leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn Unkeyed() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        <ul>
            {move || names.get()
                .into_iter()
                .map(|name| view! { <li>{name}</li> })
                .collect_view()}
        </ul>
    }
}

#[component]
fn Static() -> impl IntoView {
    view! {
        <ul>
            {["Ada", "Grace"]
                .into_iter()
                .map(|name| view! { <li>{name}</li> })
                .collect_view()}
        </ul>
    }
}

#[allow(leptos_unkeyed_reactive_collections)]
#[component]
fn Suppressed() -> impl IntoView {
    let (names, _) = signal(Vec::<String>::new());
    view! { {move || names.get().into_iter().map(|name| view! { <span>{name}</span> }).collect_view()} }
}

fn main() {}
