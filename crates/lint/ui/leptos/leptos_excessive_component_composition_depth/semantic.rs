#![allow(
    unknown_lints,
    rlib::leptos_noncanonical_view_formatting,
    rlib::leptos_overpopulated_component_modules
)]

use leptos::prelude::*;

#[component]
fn C00() -> impl IntoView {
    view! { <C01 /> }
}
#[component]
fn C01() -> impl IntoView {
    view! { <C02 /> }
}
#[component]
fn C02() -> impl IntoView {
    view! { <C03 /> }
}
#[component]
fn C03() -> impl IntoView {
    view! { <C04 /> }
}
#[component]
fn C04() -> impl IntoView {
    view! { <C05 /> }
}
#[component]
fn C05() -> impl IntoView {
    view! { <C06 /> }
}
#[component]
fn C06() -> impl IntoView {
    view! { <C07 /> }
}
#[component]
fn C07() -> impl IntoView {
    view! { <C08 /> }
}
#[component]
fn C08() -> impl IntoView {
    view! { <C09 /> }
}
#[component]
fn C09() -> impl IntoView {
    view! { <C10 /> }
}
#[component]
fn C10() -> impl IntoView {
    view! { <p>"end"</p> }
}

fn main() {}
