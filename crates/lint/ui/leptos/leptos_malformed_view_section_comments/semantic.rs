#![feature(register_tool)]
#![allow(dead_code, leptos_missing_view_section_comments, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn malformed_headings() -> impl IntoView {
    view! {
        <main>
            // ACCOUNT NAVIGATION:
            <header/>
            // Account content
            <section/>

            // Account   preferences
            <article/>

            /* Account actions */
            <footer/>

            <aside/>
            // Stranded heading
        </main>
    }
}

fn canonical_headings() -> impl IntoView {
    view! {
        <main>
            // API status
            <header/>

            // Content
            <section/>
        </main>
    }
}

#[allow(leptos_malformed_view_section_comments)]
fn suppressed() -> impl IntoView {
    view! {
        <main>
            // SHOUTING:
            <header/>
        </main>
    }
}

fn main() {}
