#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    rlib::leptos_boolean_component_props,
    rlib::leptos_implicit_default_component_props,
    rlib::leptos_manual_resource_refetch_signals,
    rlib::leptos_needlessly_cloned_signal_values,
    rlib::leptos_reactive_writes_during_view_construction,
    rlib::leptos_unkeyed_reactive_collections,
    rlib::leptos_unsanitized_inner_html,
    rlib::leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn synchronized() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| {
        target.set(source.get() * 2);
    });
}

fn through_local() {
    let source = RwSignal::new(vec![1, 2]);
    let target = RwSignal::new(Vec::<i32>::new());
    Effect::new(move |_| {
        let selected = source
            .get()
            .into_iter()
            .filter(|value| value % 2 == 0)
            .collect();
        target.set(selected);
    });
}

fn watched() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::watch(
        move || source.get(),
        move |value, _, _| target.set(*value),
        false,
    );
}

fn synchronized_sync() {
    let source = ArcRwSignal::new(1);
    let target = ArcRwSignal::new(2);
    Effect::new_sync(move |_| target.set(source.get() * 2));
}

fn synchronized_isomorphic() {
    let source = ArcRwSignal::new(1);
    let target = ArcRwSignal::new(2);
    Effect::new_isomorphic(move |_| target.set(source.get() * 2));
}

fn watched_sync() {
    let source = ArcRwSignal::new(1);
    let target = ArcRwSignal::new(2);
    Effect::watch_sync(
        move || source.get(),
        move |value, _, _| target.set(*value),
        false,
    );
}

fn deprecated_effect_functions() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    create_effect(move |_| target.set(source.get()));

    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    watch(
        move || source.get(),
        move |value, _, _| target.set(*value),
        false,
    );
}

fn external_effect() {
    let source = RwSignal::new(1);
    Effect::new(move |_| {
        println!("{}", source.get());
    });
}

fn event_callback() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| {
        let callback = move || target.set(source.get());
        let _ = callback;
    });
}

fn untracked_read() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| target.set(source.get_untracked()));
}

#[allow(rlib::leptos_effects_synchronizing_signals)]
fn suppressed() {
    let source = RwSignal::new(1);
    let target = RwSignal::new(2);
    Effect::new(move |_| target.set(source.get()));
}

fn main() {}
