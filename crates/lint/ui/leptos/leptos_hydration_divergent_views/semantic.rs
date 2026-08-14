#![allow(dead_code, unexpected_cfgs, unknown_lints)]

use leptos::prelude::*;

#[component]
fn Divergent() -> impl IntoView {
    if cfg!(target_arch = "wasm32") {
        view! { <ClientToolbar/> }.into_any()
    } else {
        view! { <ServerPlaceholder/> }.into_any()
    }
}

#[component]
fn Stable() -> impl IntoView {
    if cfg!(feature = "ssr") {
        view! { <Toolbar>"Server"</Toolbar> }.into_any()
    } else {
        view! { <Toolbar>"Browser"</Toolbar> }.into_any()
    }
}

#[component]
fn DivergentNesting() -> impl IntoView {
    if cfg!(feature = "hydrate") {
        view! { <main><span/></main> }.into_any()
    } else {
        view! { <main/><span/> }.into_any()
    }
}

#[component]
fn DivergentTextNodes() -> impl IntoView {
    if cfg!(feature = "ssr") {
        view! { <main/> }.into_any()
    } else {
        view! { <main>"Browser"</main> }.into_any()
    }
}

#[component]
fn NegatedEnvironment() -> impl IntoView {
    if !cfg!(target_arch = "wasm32") {
        view! { <ServerPlaceholder/> }.into_any()
    } else {
        view! { <ClientToolbar/> }.into_any()
    }
}

#[component]
fn MarkupInsideAttributesIsNotStructure() -> impl IntoView {
    if cfg!(feature = "hydrate") {
        view! { <main data-label="<BrowserOnly/>"/> }.into_any()
    } else {
        view! { <main data-label="<ServerOnly/>"/> }.into_any()
    }
}

#[component]
fn ClientToolbar() -> impl IntoView {
    view! { <nav/> }
}

#[component]
fn ServerPlaceholder() -> impl IntoView {
    view! { <nav/> }
}

#[component]
fn Toolbar(children: Children) -> impl IntoView {
    view! { <nav>{children()}</nav> }
}

fn main() {}
