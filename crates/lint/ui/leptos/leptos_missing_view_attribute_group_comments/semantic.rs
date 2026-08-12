#![allow(dead_code, leptos_missing_view_section_comments, unknown_lints)]

use leptos::prelude::*;

#[component]
fn AttributeGroups() -> impl IntoView {
    let pending = RwSignal::new(false);
    view! {
        <button
            type="submit"
            form="profile"
            class="primary"
            class:pending=move || pending.get()
            disabled=move || pending.get()
            aria-busy=move || pending.get()
            on:click=move |_| pending.set(true)
        >
            "Ungrouped"
        </button>

        <button
            // Submission identity
            type="submit"
            form="profile"

            // Availability and progress
            disabled=move || pending.get()
            aria-busy=move || pending.get()

            // Submission presentation and behavior
            class="primary"
            class:pending=move || pending.get()
            on:click=move |_| pending.set(true)
        >
            "Grouped"
        </button>
    }
}

fn main() {}
