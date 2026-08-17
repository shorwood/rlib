#![allow(unknown_lints, leptos_noncanonical_view_formatting)]

use leptos::prelude::*;

#[component]
fn DeepView() -> impl IntoView {
    view! { <main><section><div><div><div><div><div><span>"deep"</span></div></div></div></div></div></section></main> }
}

fn main() {}
