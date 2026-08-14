#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(clippy::unused_async)]
#![allow(
    dead_code,
    deprecated,
    leptos_boolean_component_props,
    leptos_implicit_default_component_props,
    leptos_reactive_writes_during_view_construction,
    leptos_writable_signal_component_props
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

async fn load(value: u32) -> u32 {
    value
}

fn manual_refetch() {
    let (revision, set_revision) = signal(0_u32);
    let resource = LocalResource::new(move || {
        revision.get();
        load(0)
    });
    set_revision.update(|value| *value += 1);
    let _ = resource;
}

fn explicit_discards() {
    let (revision, _) = signal(0_u32);
    let wildcard = LocalResource::new(move || {
        let _ = revision.get();
        load(0)
    });

    let (revision, _) = signal(String::new());
    let dropped = LocalResource::new(move || {
        drop(revision.get());
        load(0)
    });
    let _ = (wildcard, dropped);
}

fn arc_local_resource() {
    let revision = ArcRwSignal::new(0_u32);
    let resource = ArcLocalResource::new(move || {
        revision.get();
        load(0)
    });
    let _ = resource;
}

fn meaningful_dependency() {
    let (user_id, _) = signal(1_u32);
    let resource = LocalResource::new(move || load(user_id.get()));
    let _ = resource;
}

fn non_trigger_reads() {
    let (revision, _) = signal(0_u32);
    revision.get();

    let resource = LocalResource::new(move || {
        revision.get_untracked();
        let callback = move || revision.get();
        let _ = callback;
        load(0)
    });
    let _ = resource;
}

#[allow(leptos_manual_resource_refetch_signals)]
fn suppressed() {
    let (revision, _) = signal(0_u32);
    let resource = LocalResource::new(move || {
        revision.get();
        load(0)
    });
    let _ = resource;
}

fn main() {}
