#![allow(
    dead_code,
    leptos_mismatched_view_attribute_groups,
    leptos_missing_view_section_comments,
    leptos_oversized_view_attribute_groups,
    unknown_lints
)]

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

        <button
            // Submission contract
            type="submit"
            form="profile"
            class="primary"
            class:pending=move || pending.get()
            disabled=move || pending.get()
            aria-busy=move || pending.get()
            on:click=move |_| pending.set(true)
        >
            "Only one group"
        </button>

        <button
            type="submit"
            form="profile"
            class="primary"
            style="display: block"
            disabled=move || pending.get()
            aria-busy=move || pending.get()
        >
            "At the complexity boundary"
        </button>

        <div
            class:first=true
            class:second=true
            class:third=true
            class:fourth=true
            class:fifth=true
            class:sixth=true
            class:seventh=true
        />

        <button
            // Submission identity
            type="submit"
            form="profile"

            // Remaining contract
            class="primary"
            class:pending=move || pending.get()
            disabled=move || pending.get()
            aria-busy=move || pending.get()
            on:click=move |_| pending.set(true)
        >
            "Exactly two groups"
        </button>
    }
}

fn main() {}
