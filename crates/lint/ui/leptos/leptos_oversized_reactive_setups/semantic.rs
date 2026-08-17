#![allow(
    unknown_lints,
    leptos_noncanonical_view_formatting,
    leptos_fragmented_reactive_state
)]

use leptos::prelude::*;

#[component]
fn CrowdedSetup() -> impl IntoView {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    let g = 7;
    let h = 8;
    let i = 9;
    view! { <p>{a+b+c+d+e+f+g+h+i}</p> }
}

fn main() {}
