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

fn fragments_share_the_parent_scope() -> impl IntoView {
    view! {
        <>
            // Primary navigation
            <nav/>
        </>

        // Primary navigation
        <aside/>
    }
}

fn separate_view_calls() -> impl IntoView {
    let first = view! {
        // Account content
        <header/>
    };
    let second = view! {
        // Account content
        <section/>
    };
    (first, second)
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
