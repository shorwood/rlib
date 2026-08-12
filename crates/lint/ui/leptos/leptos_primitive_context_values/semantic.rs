#![allow(dead_code, unknown_lints)]

use leptos::prelude::*;

#[derive(Clone, Copy)]
struct ThemeContext(WriteSignal<bool>);

fn context_values() {
    let (_, set_theme) = signal(false);
    provide_context(set_theme);
    let _ambiguous = use_context::<WriteSignal<bool>>();

    provide_context(ThemeContext(set_theme));
    let _branded = use_context::<ThemeContext>();
}

fn main() {}
