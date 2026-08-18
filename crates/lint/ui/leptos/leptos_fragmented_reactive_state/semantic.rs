#![allow(
    unknown_lints,
    rlib::leptos_noncanonical_view_formatting,
    rlib::leptos_oversized_reactive_setups
)]

use leptos::prelude::*;

#[component]
fn Fragmented() -> impl IntoView {
    let a = RwSignal::new(1);
    let b = RwSignal::new(2);
    let c = RwSignal::new(3);
    let d = RwSignal::new(4);
    let e = RwSignal::new(5);
    view! { <p>{move || a.get()+b.get()+c.get()+d.get()+e.get()}</p> }
}

fn main() {}
