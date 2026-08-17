#![allow(unknown_lints, leptos_noncanonical_view_formatting)]

use leptos::prelude::*;
use leptos::task::spawn_local as detach;

#[component]
fn Detached() -> impl IntoView {
    detach(async move {});
    view! { <p>"waiting"</p> }
}

fn main() {}
