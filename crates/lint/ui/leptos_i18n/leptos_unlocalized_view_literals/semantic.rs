#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn Card(#[prop(into)] label: TextProp, #[prop(into)] description: TextProp) -> impl IntoView {
    view! { <article>{move || label.get()}{move || description.get()}</article> }
}

fn localized_text() -> String {
    "catalog value".to_owned()
}

fn unlocalized() -> impl IntoView {
    view! {
        <main aria-label="Migration dashboard">
            <input placeholder="Search systems…" title="System search"/>
            <img alt="Campaign progress"/>
            <button><span/>"Save changes"</button>
            <Card label="Attention" description="Items needing action"/>
            <div data-label="Owner">"Assigned team"</div>
        </main>
    }
}

fn localized_or_technical() -> impl IntoView {
    view! {
        <main id="dashboard" role="main" aria-controls="results" data-state="ready">
            <a href="/systems" title=localized_text()>{localized_text()}</a>
            <input type="search" value="open" placeholder=localized_text()/>
            <Card label=localized_text() description=localized_text()/>
        </main>
    }
}

fn main() {}
