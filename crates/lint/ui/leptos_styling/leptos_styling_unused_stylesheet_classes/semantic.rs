#![allow(
    leptos_noncanonical_view_formatting,
    leptos_styling_inline_style_properties,
    leptos_styling_non_colocated_component_styles,
    leptos_styling_noncanonical_css,
    leptos_styling_unscoped_component_selectors,
    leptos_styling_untyped_component_classes,
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
    "ui/leptos_styling/leptos_styling_unused_stylesheet_classes/semantic.css",
    "ui"
);

fn main() {
    let _ = view! { <main class=style::ROOT>"Page"</main> };
}
