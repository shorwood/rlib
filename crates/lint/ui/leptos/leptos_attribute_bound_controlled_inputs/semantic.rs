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

struct FormState {
    name: RwSignal<String>,
}

fn field_value() {
    let state = FormState {
        name: RwSignal::new(String::new()),
    };
    let _ = view! { <input value=state.name/> };
}

fn spaced_attribute() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input value = name/> };
}

fn long_multiline_tag() {
    let name = RwSignal::new(String::new());
    let _ = view! {
        <input
            aria-label="a deliberately long label that pushes the tag opening beyond the old fixed source window"
            class="controlled-input controlled-input--with-a-very-long-class-name"
            value=name
        />
    };
}

fn similarly_named_attribute() {
    let name = RwSignal::new(String::new());
    let _ = view! { <input data-value=name/> };
}

fn read_only_signal() {
    let (name, _) = signal(String::new());
    let _ = view! { <input value=name/> };
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
