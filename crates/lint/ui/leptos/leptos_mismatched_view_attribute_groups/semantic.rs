#![allow(dead_code, unknown_lints)]

use leptos::prelude::*;

#[component]
fn MismatchedGroups() -> impl IntoView {
    view! {
        <button
            // Submission presentation
            class="primary"
            on:click=move |_| {}
        >
            "Save"
        </button>
        <button
            // Submission behavior
            on:click=move |_| {}
        >
            "Cancel"
        </button>
    }
}

fn main() {}
