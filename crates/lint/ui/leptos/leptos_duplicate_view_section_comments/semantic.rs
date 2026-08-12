#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn duplicate_headings() -> impl IntoView {
    view! {
        <main>
            // Account content
            <header/>

            // Account content
            <section/>
        </main>
    }
}

fn separate_nested_scopes() -> impl IntoView {
    view! {
        <main>
            // Account content
            <header/>

            // Account details
            <section>
                // Account content
                <article/>
            </section>
        </main>
    }
}

#[allow(leptos_duplicate_view_section_comments)]
fn suppressed() -> impl IntoView {
    view! {
        <main>
            // Account content
            <header/>

            // Account content
            <section/>
        </main>
    }
}

fn main() {}
