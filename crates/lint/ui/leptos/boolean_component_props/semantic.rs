#![feature(register_tool)]
#![allow(dead_code, deprecated, unknown_lints, writable_signal_component_props)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

type Flag = bool;

enum AccountStatus {
    Active,
    Inactive,
}

struct CustomState<T>(T);

#[component]
fn Direct(active: bool, aliased: Flag) -> impl IntoView {
    view! { <span>{active.then_some(aliased)}</span> }
}

#[component]
fn Wrapped(
    optional: Option<bool>,
    signal: Signal<bool>,
    read_signal: ReadSignal<bool>,
    rw_signal: RwSignal<bool>,
    maybe_signal: MaybeSignal<bool>,
    nested: Option<Signal<bool>>,
) -> impl IntoView {
    let _ = (
        optional,
        signal,
        read_signal,
        rw_signal,
        maybe_signal,
        nested,
    );
    view! { <span/> }
}

// Domain states, callback booleans, and unknown wrappers are outside this rule.
#[component]
fn Valid(
    status: AccountStatus,
    predicate: Callback<u8, bool>,
    event: Callback<bool>,
    custom: CustomState<bool>,
) -> impl IntoView {
    let _ = (status, predicate, event, custom);
    view! { <button disabled=true/> }
}

#[allow(boolean_component_props)]
#[component]
fn Suppressed(active: bool) -> impl IntoView {
    view! { <span>{active}</span> }
}

fn main() {}
