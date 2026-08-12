#![feature(register_tool)]
#![allow(
    clippy::absolute_paths,
    unknown_lints,
    leptos_boolean_component_props,
    dead_code,
    deprecated,
    leptos_implicit_default_component_props,
    leptos_writable_signal_component_props
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn SuspendedWrite() -> impl IntoView {
    let (count, set_count) = signal(0_u32);
    set_count.set(1);

    view! {
        <Suspense fallback=move || ()>
            {move || Suspend::new(async move {
                set_count.set(2);
                set_count.update(|value| *value += 1);

                view! {
                    <button on:click=move |_| set_count.set(3)>
                        {move || count.get()}
                    </button>
                }
            })}
        </Suspense>
    }
}

#[allow(leptos_reactive_writes_during_view_construction)]
#[component]
fn Suppressed() -> impl IntoView {
    let (_, set_count) = signal(0_u32);
    view! {
        <Suspense fallback=move || ()>
            {move || Suspend::new(async move {
                set_count.set(1);
                view! { <span/> }
            })}
        </Suspense>
    }
}

fn main() {}
