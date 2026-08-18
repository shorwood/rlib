#![allow(unknown_lints, rlib::leptos_noncanonical_view_formatting)]

use leptos::prelude::*;

#[component]
fn RepeatedFields() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    view! {
        <label class="field"><span>"Name"</span><input prop:value=name /><small>"Required"</small></label>
        <label class="field"><span>"Email"</span><input prop:value=email /><small>"Required"</small></label>
    }
}

fn main() {}
