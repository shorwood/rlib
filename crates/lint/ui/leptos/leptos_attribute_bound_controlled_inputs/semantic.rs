#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn attribute_value() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input value=name/> };
}

fn attribute_checked() {
    let enabled = RwSignal::new(false);
    let _ = view! { <input type="checkbox" checked=enabled/> };
}

fn property_value() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input prop:value=move || name.get()/> };
}

fn bound_value() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input bind:value=name/> };
}

fn fixed_initial_value() {
    let _ = view! { <input value="initial"/> };
}

#[allow(leptos_attribute_bound_controlled_inputs)]
fn suppressed() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input value=name/> };
}

fn main() {}
