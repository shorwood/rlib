#![feature(register_tool)]
#![allow(dead_code, unknown_lints, leptos_duplicate_view_section_comments)]
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

fn repeated_markup() -> impl IntoView {
    view! {
        <main>
            // Navigation
            <Navigation/>

            // Submit button
            <button>"Submit"</button>

            // Navigation
            <nav/>
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

#[allow(leptos_markup_repeating_view_comments)]
fn suppressed() -> impl IntoView {
    view! {
        // Navigation
        <Navigation/>
    }
}

fn main() {}
