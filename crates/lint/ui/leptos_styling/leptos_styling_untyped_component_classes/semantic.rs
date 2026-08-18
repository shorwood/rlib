#![allow(
    rlib::leptos_noncanonical_view_formatting,
    rlib::leptos_styling_inline_style_properties,
    rlib::leptos_styling_non_colocated_component_styles,
    rlib::leptos_styling_noncanonical_css,
    rlib::leptos_styling_unscoped_component_selectors,
    rlib::leptos_styling_unused_stylesheet_classes,
    unknown_lints
)]

use leptos::prelude::*;

macro_rules! style_sheet {
    ($name:ident, $path:literal, $output:literal) => {
        mod $name {
            pub const ROOT: &str = "root";
        }
    };
}

style_sheet!(
    style,
    "ui/leptos_styling/leptos_styling_untyped_component_classes/semantic.css",
    "ui"
);

fn main() {
    let _ = view! { <main class="root">{view! { <span class="nested">"Page"</span> }}</main> };
}
