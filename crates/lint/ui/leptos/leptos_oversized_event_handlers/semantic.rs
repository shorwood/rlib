#![allow(unknown_lints, leptos_noncanonical_view_formatting)]

use leptos::prelude::*;

#[component]
fn BusyButton() -> impl IntoView {
    view! { <button on:click=move |_| { let a=1; let b=2; let c=3; let _d=4; let _=a+b+c; }>"Save"</button> }
}

fn main() {}
