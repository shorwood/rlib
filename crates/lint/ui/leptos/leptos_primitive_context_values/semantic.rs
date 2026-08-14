#![allow(dead_code, unknown_lints)]

use leptos::prelude::*;

#[derive(Clone, Copy)]
struct ThemeContext(WriteSignal<bool>);

fn context_values() {
    let (_, set_theme) = signal(false);
    provide_context(set_theme);
    let _ambiguous = use_context::<WriteSignal<bool>>();
    let _expected = expect_context::<WriteSignal<bool>>();
    let _taken = take_context::<WriteSignal<bool>>();
    let _borrowed = with_context::<WriteSignal<bool>, _>(|_| ());
    let _updated = update_context::<WriteSignal<bool>, _>(|_| ());
    let owner = Owner::new();
    let _bidirectional = owner.use_context_bidirectional::<WriteSignal<bool>>();

    provide_context(ThemeContext(set_theme));
    let _branded = use_context::<ThemeContext>();
    let _branded_take = take_context::<ThemeContext>();
    let _branded_borrow = with_context::<ThemeContext, _>(|_| ());
    let _branded_bidirectional = owner.use_context_bidirectional::<ThemeContext>();
}

fn main() {}
