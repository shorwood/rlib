#![allow(dead_code, unknown_lints)]

use leptos::prelude::*;

#[component]
fn OversizedGroup() -> impl IntoView {
    view! {
        <button
            // Button configuration
            id="save"
            type="submit"
            form="profile"
            class="primary"
            disabled=false
        >
            "Save"
        </button>
        <button
            // Submission identity
            id="cancel"
            type="button"

            // Visual treatment
            class="secondary"
            style:color="gray"
        >
            "Cancel"
        </button>
    }
}

fn main() {}
