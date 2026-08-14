#![allow(dead_code, unknown_lints, leptos_missing_view_section_comments)]

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
        <button
            // Accessibility
            aria-label="Continue"
            class="primary"
        >
            "Continue"
        </button>
        <button
            // Database connection
            class="database"
        >
            "Connect"
        </button>
        <button
            // Statement formatting
            class="statement"
        >
            "Format"
        </button>
    }
}

fn main() {}
