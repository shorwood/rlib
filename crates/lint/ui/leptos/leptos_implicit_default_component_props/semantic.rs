#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(
    dead_code,
    deprecated,
    leptos_writable_signal_component_props
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[derive(Clone, Default)]
enum DrawerWidth {
    #[default]
    Standard,
    Wide,
}

#[component]
fn Implicit(
    #[prop(optional)] count: usize,
    #[prop(optional)] label: String,
    #[prop(optional)] width: DrawerWidth,
) -> impl IntoView {
    let _ = (count, label, width);
    view! { <span/> }
}

#[component]
fn Explicit(
    #[prop(default = 100)] max: usize,
    #[prop(default = DrawerWidth::Standard)] width: DrawerWidth,
    #[prop(optional)] subtitle: Option<String>,
) -> impl IntoView {
    let _ = (max, width, subtitle);
    view! { <span/> }
}

#[allow(leptos_implicit_default_component_props)]
#[component]
fn Suppressed(#[prop(optional)] count: usize) -> impl IntoView {
    view! { <span>{count}</span> }
}

fn main() {}
