#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn oversized_section() -> impl IntoView {
    view! {
        <main>
            // Account workspace
            <header on:click=move |_| {}/>
            <nav/>
            <Show when=move || true><section/></Show>
        </main>
    }
}

fn bounded_sections() -> impl IntoView {
    view! {
        <main>
            // Account navigation
            <header on:click=move |_| {}/>
            <nav/>

            // Account content
            <Show when=move || true><section/></Show>
        </main>
    }
}

#[allow(leptos_oversized_view_sections)]
fn suppressed() -> impl IntoView {
    view! {
        <main>
            // Account workspace
            <header on:click=move |_| {}/>
            <nav/>
            <Show when=move || true><section/></Show>
        </main>
    }
}

fn main() {}
