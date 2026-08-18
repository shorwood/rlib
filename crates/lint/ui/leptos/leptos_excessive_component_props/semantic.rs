#![allow(unknown_lints, rlib::leptos_noncanonical_view_formatting)]

use leptos::prelude::*;

#[component]
fn PropHeavy(
    a: String,
    b: String,
    c: String,
    d: String,
    e: String,
    f: String,
    g: String,
) -> impl IntoView {
    view! { <p>{a}{b}{c}{d}{e}{f}{g}</p> }
}

fn main() {}
