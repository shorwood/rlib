#![feature(register_tool)]
#![allow(clippy::option_option)] // Nested transparent wrappers are intentional lint coverage.
#![allow(unknown_lints)]
#![allow(boolean_component_props, dead_code, deprecated, unused_variables)]
#![register_tool(rlib_lint)]

use std::ops::DerefMut;
use std::panic::Location;

use leptos::prelude::*;
use reactive_stores::Store;

type Writer<T> = WriteSignal<T>;

const fn pass<T>(value: T) -> T {
    value
}

#[derive(Clone)]
struct CustomWriter<T>(ArcRwSignal<T>);

impl<T> DefinedAt for CustomWriter<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.0.defined_at()
    }
}

impl<T> Notify for CustomWriter<T> {
    fn notify(&self) {
        self.0.notify();
    }
}

impl<T: 'static> Write for CustomWriter<T> {
    type Value = T;

    fn try_write(&self) -> Option<impl UntrackableGuard<Target = Self::Value>> {
        self.0.try_write()
    }

    fn try_write_untracked(&self) -> Option<impl DerefMut<Target = Self::Value>> {
        self.0.try_write_untracked()
    }
}

#[component]
fn Concrete(
    write: WriteSignal<String>,
    rw: RwSignal<String>,
    arc_write: ArcWriteSignal<String>,
    arc_rw: ArcRwSignal<String>,
    alias: Writer<String>,
    store: Store<String>,
    custom: CustomWriter<String>,
    optional: Option<Option<RwSignal<String>>>,
) -> impl IntoView {
    view! { <span/> }
}

#[component]
fn Generic<W>(writer: W) -> impl IntoView
where
    W: Write + Send + Sync + 'static,
{
    view! { <span/> }
}

#[component]
fn BoundValue(value: RwSignal<String>) -> impl IntoView {
    view! { <input bind:value=value/> }
}

#[component]
fn BoundChecked(checked: RwSignal<bool>) -> impl IntoView {
    view! { <input type="checkbox" bind:checked=checked/> }
}

#[component]
fn BoundGroup(read: ReadSignal<String>, write: WriteSignal<String>) -> impl IntoView {
    view! { <input type="radio" bind:group=(read, write)/> }
}

#[component]
fn BoundAlias(value: RwSignal<String>) -> impl IntoView {
    let forwarded = value;
    view! { <input bind:value=forwarded/> }
}

#[component]
fn ReadAndBind(value: RwSignal<String>) -> impl IntoView {
    let snapshot = value.get();
    view! { <input bind:value=value/><span>{snapshot}</span> }
}

#[component]
fn CaptureAndBind(value: RwSignal<String>) -> impl IntoView {
    view! {
        <input bind:value=value/>
        <button on:click=move |_| value.set(String::new())>"Reset"</button>
    }
}

#[component]
fn MultipleBindings(value: RwSignal<String>) -> impl IntoView {
    view! { <input bind:value=value/><input bind:value=value/> }
}

#[component]
fn ProjectedBinding(value: RwSignal<String>) -> impl IntoView {
    view! { <input bind:value=pass(value)/> }
}

#[component]
fn TransparentChild(value: RwSignal<String>) -> impl IntoView {
    view! { <input bind:value=value/> }
}

#[component]
fn ForwardedToChild(value: RwSignal<String>) -> impl IntoView {
    view! { <TransparentChild value=value/> }
}

#[component]
fn Valid(
    read: ReadSignal<String>,
    signal: Signal<String>,
    memo: Memo<String>,
    callback: Callback<String>,
    wrapped: Vec<RwSignal<String>>,
) -> impl IntoView {
    view! { <span/> }
}

#[allow(writable_signal_component_props)]
#[component]
fn Suppressed(value: RwSignal<String>) -> impl IntoView {
    view! { <span/> }
}

fn main() {}
