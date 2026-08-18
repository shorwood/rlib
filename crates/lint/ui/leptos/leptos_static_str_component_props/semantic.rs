#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

type StaticText = &'static str;

struct TextCarrier {
    label: StaticText,
}

enum NoticeStatus {
    Informational,
    Warning,
}

#[slot]
struct StaticTextSlot {
    label: &'static str,
}

#[component]
fn StaticTextProps(
    direct: &'static str,
    aliased: StaticText,
    optional: Option<&'static str>,
    signal: Signal<&'static str>,
    callback: Callback<&'static str>,
    tuple: (u8, &'static str),
    carrier: TextCarrier,
) -> impl IntoView {
    let _ = (direct, aliased, optional, signal, callback, tuple, carrier);
    view! { <span/> }
}

#[component]
fn LocalizableProps(
    label: TextProp,
    technical_value: String,
    status: NoticeStatus,
) -> impl IntoView {
    let _ = (label, technical_value, status);
    view! { <span/> }
}

fn internal_policy_value() -> &'static str {
    "technical"
}

fn main() {}
