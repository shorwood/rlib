#![feature(register_tool)]
#![allow(dead_code, unknown_lints, rlib::leptos_duplicate_view_section_comments)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn Navigation() -> impl IntoView {
    view! { <nav/> }
}

#[component]
fn Divider() -> impl IntoView {
    view! { <hr/> }
}

mod components {
    pub(super) use super::Navigation;
}

fn repeated_markup() -> impl IntoView {
    view! {
        <main>
            // Navigation
            <Navigation/>

            // Submit button
            <button>"Submit"</button>

            // Navigation
            <nav/>

            // Navigation
            <components::Navigation/>

            // Account status
            <h2>"Account status"</h2>
        </main>
    }
}

fn informative_or_ambiguous() -> impl IntoView {
    let label = "Save";
    view! {
        <main>
            // Account navigation
            <Navigation/>

            // Navigation
            <Navigation/><Divider/>

            // Primary action
            <button>{label}</button>
        </main>
    }
}

#[allow(rlib::leptos_markup_repeating_view_comments)]
fn suppressed() -> impl IntoView {
    view! {
        // Navigation
        <Navigation/>
    }
}

fn main() {}
