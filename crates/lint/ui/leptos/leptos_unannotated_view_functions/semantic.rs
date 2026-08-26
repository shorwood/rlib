#![feature(register_tool)]
#![allow(dead_code, non_snake_case)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

type ErasedView = AnyView;

fn snake_view() -> impl IntoView {
    view! { <span>"snake"</span> }
}

fn PascalView() -> AnyView {
    view! { <span>"pascal"</span> }.into_any()
}

fn aliased_view() -> ErasedView {
    view! { <span>"alias"</span> }.into_any()
}

fn concrete_view() -> View<&'static str> {
    "concrete".into_view()
}

fn forwarded_view(view: AnyView) -> AnyView {
    view
}

#[component]
fn Annotated() -> impl IntoView {
    view! { <span>"component"</span> }
}

#[component(transparent)]
fn Transparent() -> impl IntoView {
    view! { <span>"transparent"</span> }
}

fn concrete_renderable() -> &'static str {
    "not an explicit view contract"
}

async fn asynchronous_view() -> AnyView {
    view! { <span>"async"</span> }.into_any()
}

#[allow(improper_ctypes_definitions)]
extern "C" fn foreign_view() -> AnyView {
    view! { <span>"foreign ABI"</span> }.into_any()
}

struct Renderer;

impl Renderer {
    fn method(&self) -> AnyView {
        view! { <span>"method"</span> }.into_any()
    }
}

#[allow(rlib::leptos_unannotated_view_functions)]
fn suppressed() -> AnyView {
    view! { <span>"suppressed"</span> }.into_any()
}

fn main() {}
