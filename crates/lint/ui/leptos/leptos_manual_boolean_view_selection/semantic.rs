#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn conditional(flag: bool) -> AnyView {
    if flag {
        view! { <span>"configured"</span> }.into_any()
    } else {
        view! { <span>"empty"</span> }.into_any()
    }
}

fn matched(flag: bool) -> AnyView {
    match flag {
        true => view! { <strong>"yes"</strong> }.into_any(),
        false => view! { <em>"no"</em> }.into_any(),
    }
}

fn wildcard(flag: bool) -> AnyView {
    match flag {
        false => view! { <small>"no"</small> }.into_any(),
        _ => view! { <strong>"yes"</strong> }.into_any(),
    }
}

fn boolean_helpers(flag: bool) {
    let _lazy = flag.then(|| view! { <span>"lazy"</span> }.into_any());
    let _eager = flag.then_some(view! { <span>"eager"</span> }.into_any());
    let _ufcs_lazy = bool::then(flag, || view! { <span>"ufcs lazy"</span> }.into_any());
    let _ufcs_eager = bool::then_some(flag, view! { <span>"ufcs eager"</span> }.into_any());
}

fn reactive() -> impl IntoView {
    let flag = RwSignal::new(false);
    view! {
        {move || if flag.get() {
            view! { <span>"on"</span> }.into_any()
        } else {
            view! { <span>"off"</span> }.into_any()
        }}
    }
}

fn declarative(flag: bool) -> impl IntoView {
    view! {
        <Show when=move || flag fallback=|| view! { <span>"off"</span> }>
            <span>"on"</span>
        </Show>
    }
}

fn data_selection(flag: bool) {
    let _number = if flag { 1 } else { 2 };
    let _word = match flag {
        true => "yes",
        false => "no",
    };
    let _optional = flag.then_some(1_u8);
}

fn optional_binding(value: Option<String>) -> AnyView {
    if let Some(value) = value {
        view! { <span>{value}</span> }.into_any()
    } else {
        view! { <span>"empty"</span> }.into_any()
    }
}

struct ForeignBool(bool);

impl ForeignBool {
    fn then(self, render: impl FnOnce() -> AnyView) -> Option<AnyView> {
        if self.0 { Some(render()) } else { None }
    }

    fn then_some(self, view: AnyView) -> Option<AnyView> {
        if self.0 { Some(view) } else { None }
    }
}

fn foreign_helpers() {
    let _ = ForeignBool(true).then(|| view! { <span>"foreign"</span> }.into_any());
    let _ = ForeignBool(true).then_some(view! { <span>"foreign"</span> }.into_any());
}

#[allow(rlib::leptos_manual_boolean_view_selection)]
fn suppressed(flag: bool) -> AnyView {
    if flag {
        view! { <span>"yes"</span> }.into_any()
    } else {
        view! { <span>"no"</span> }.into_any()
    }
}

fn main() {}
