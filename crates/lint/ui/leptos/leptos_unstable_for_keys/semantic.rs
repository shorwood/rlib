#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[derive(Clone)]
struct Item {
    id: u64,
    name: String,
}

fn constant_key() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For each=move || items.get() key=|_| 0 children=|item| item.name/>
    }
}

fn index_key() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For
            each=move || items.get().into_iter().enumerate()
            key=|(index, _)| *index
            children=|(_, item)| item.name
        />
    }
}

fn renamed_index_key() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For
            each=move || items.get().into_iter().enumerate()
            key=|(offset, _)| *offset
            children=|(_, item)| item.name
        />
    }
}

fn constant_string_key() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For each=move || items.get() key = |item| { let _ = item.id; "same" } children=|item| item.name/>
    }
}

fn stable_key() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For each=move || items.get() key=|item| item.id children=|item| item.name/>
    }
}

fn domain_value_named_index() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! {
        <For each=move || items.get() key=|index| index.id children=|item| item.name/>
    }
}

#[allow(rlib::leptos_unstable_for_keys)]
fn suppressed() -> impl IntoView {
    let items = RwSignal::new(Vec::<Item>::new());
    view! { <For each=move || items.get() key=|_| true children=|item| item.name/> }
}

fn main() {}
