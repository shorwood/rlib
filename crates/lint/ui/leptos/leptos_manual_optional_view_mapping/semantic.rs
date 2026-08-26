#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

fn mapped(value: Option<String>) -> Option<AnyView> {
    value.map(|value| view! { <span>{value}</span> }.into_any())
}

fn eager_fallback(value: Option<String>) -> AnyView {
    value.map_or(view! { <span>"empty"</span> }.into_any(), |value| {
        view! { <strong>{value}</strong> }.into_any()
    })
}

fn lazy_fallback(value: Option<String>) -> AnyView {
    value.map_or_else(
        || view! { <span>"empty"</span> }.into_any(),
        |value| view! { <strong>{value}</strong> }.into_any(),
    )
}

fn ufcs(value: Option<String>) {
    let _mapped = Option::map(value.clone(), |value| {
        view! { <span>{value}</span> }.into_any()
    });
    let _eager = Option::map_or(
        value.clone(),
        view! { <span>"empty"</span> }.into_any(),
        |value| view! { <strong>{value}</strong> }.into_any(),
    );
    let _lazy = Option::map_or_else(
        value,
        || view! { <span>"empty"</span> }.into_any(),
        |value| view! { <strong>{value}</strong> }.into_any(),
    );
}

struct NonClone(String);

fn non_clone(value: Option<NonClone>) -> Option<AnyView> {
    value.map(|value| view! { <span>{value.0}</span> }.into_any())
}

fn declarative(value: Option<String>) -> impl IntoView {
    view! {
        <ShowLet some=value let:value fallback=|| view! { <span>"empty"</span> }>
            <strong>{value}</strong>
        </ShowLet>
    }
}

fn data_mappings(value: Option<String>, result: Result<String, ()>) {
    let _length = value.map(|value| value.len());
    let _result = result.map(|value| view! { <span>{value}</span> }.into_any());
}

struct ForeignOption(Option<String>);

impl ForeignOption {
    fn map(self, render: impl FnOnce(String) -> AnyView) -> Option<AnyView> {
        match self.0 {
            Some(value) => Some(render(value)),
            None => None,
        }
    }

    fn map_or_else(
        self,
        fallback: impl FnOnce() -> AnyView,
        render: impl FnOnce(String) -> AnyView,
    ) -> AnyView {
        match self.0 {
            Some(value) => render(value),
            None => fallback(),
        }
    }
}

fn foreign_mappings(value: ForeignOption) {
    let _ = value.map(|value| view! { <span>{value}</span> }.into_any());
    let _ = ForeignOption(None).map_or_else(
        || view! { <span>"empty"</span> }.into_any(),
        |value| view! { <span>{value}</span> }.into_any(),
    );
}

#[allow(rlib::leptos_manual_optional_view_mapping)]
fn suppressed(value: Option<String>) -> Option<AnyView> {
    value.map(|value| view! { <span>{value}</span> }.into_any())
}

fn main() {}
