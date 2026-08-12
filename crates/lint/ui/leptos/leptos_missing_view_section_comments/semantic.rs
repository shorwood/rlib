#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn unnamed_complex_scope() -> impl IntoView {
    view! {
        <main>
            <header>"Account"</header>
            <nav>"Navigation"</nav>
            <section>"Summary"</section>
            <section>"Activity"</section>
            <footer>"Actions"</footer>
        </main>
    }
}

fn exact_threshold_is_allowed() -> impl IntoView {
    view! { <><header/><nav/><main/><footer/></> }
}

fn named_complex_scope() -> impl IntoView {
    view! {
        <main>
            // Account navigation
            <header/>
            <nav/>

            // Account content
            <section/>
            <article/>
            <footer/>
        </main>
    }
}

#[allow(leptos_missing_view_section_comments)]
fn suppressed() -> impl IntoView {
    view! { <><header/><nav/><main/><aside/><footer/></> }
}

fn main() {}
