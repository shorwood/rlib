#![allow(unknown_lints, rlib::leptos_noncanonical_view_formatting)]

use leptos::prelude::*;

fn editor_state() -> RwSignal<String> {
    RwSignal::new(String::new())
}

#[component]
fn Editor() -> impl IntoView {
    let draft = editor_state();
    view! { <input prop:value=move || draft.get() /> }
}

fn main() {}
