#![feature(register_tool)]
#![allow(dead_code, rlib::leptos_malformed_view_section_comments, unknown_lints)]
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

fn malformed_heading_does_not_name_the_region() -> impl IntoView {
    view! {
        <main>
            // ACCOUNT REGION:
            <header/>
            <nav/>
            <section/>
            <article/>
            <footer/>
        </main>
    }
}

fn later_heading_does_not_name_the_first_region() -> impl IntoView {
    view! {
        <main>
            <header/>

            // Remaining content
            <nav/>
            <section/>
            <article/>
            <footer/>
        </main>
    }
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

#[allow(rlib::leptos_missing_view_section_comments)]
fn suppressed() -> impl IntoView {
    view! { <><header/><nav/><main/><aside/><footer/></> }
}

fn main() {}
