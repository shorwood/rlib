#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

mod controls {
    pub(super) use leptos::prelude::Show;
}

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

fn qualified_control_section() -> impl IntoView {
    view! {
        <main>
            // Qualified account workspace
            <header on:click=move |_| {}/>
            <nav/>
            <controls::Show when=move || true><section/></controls::Show>
        </main>
    }
}

#[allow(rlib::leptos_oversized_view_sections)]
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
