# Leptos Styling lints

## Summary

Rules for colocated, scoped, typed, and consistently formatted Leptos component styles.

## How to enable

Enable the `leptos_styling` Cargo feature to register this family. Enable the whole family with `rlib::leptos_styling`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::leptos_styling_inline_style_properties`](./leptos_styling_inline_style_properties/README.md) | Warns about direct inline CSS properties in Leptos views while allowing dynamic values to cross into the paired stylesheet through custom properties. | Code clarity | Manual |
| [`rlib::leptos_styling_non_colocated_component_styles`](./leptos_styling_non_colocated_component_styles/README.md) | Requires every styled Leptos source module to declare exactly one external `leptos_styling` stylesheet with the same stem and the local alias `style`. | Code clarity | Manual |
| [`rlib::leptos_styling_noncanonical_css`](./leptos_styling_noncanonical_css/README.md) | Strictly parses registered CSS files and requires their source to match deterministic Malva output. | Style | Automatic |
| [`rlib::leptos_styling_unscoped_component_selectors`](./leptos_styling_unscoped_component_selectors/README.md) | Requires every ordinary selector branch in a paired component stylesheet to be anchored by a class from that stylesheet and forbids stylesheet imports. | Code clarity | Manual |
| [`rlib::leptos_styling_untyped_component_classes`](./leptos_styling_untyped_component_classes/README.md) | Requires Leptos class values to be composed from generated constants belonging to the paired local stylesheet. | Code clarity | Manual |
| [`rlib::leptos_styling_unused_stylesheet_classes`](./leptos_styling_unused_stylesheet_classes/README.md) | Finds class selectors in a paired component stylesheet that have no corresponding typed reference in the owning Rust module. | Code clarity | Manual |
