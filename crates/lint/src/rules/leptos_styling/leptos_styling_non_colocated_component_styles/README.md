# leptos_styling_non_colocated_component_styles

## What it does

Requires every styled Leptos source module to declare exactly one external `leptos_styling`
stylesheet with the same stem and the local alias `style`.

## Why is this bad?

Shared, foreign, or inline style sources hide component ownership and make style changes ripple across
unrelated markup.

## Example

```rust
leptos_styling::style_sheet!(shared, "src/styles/app.css", "app");
view! { <main class=shared::PAGE /> }
```

## Use instead

```rust
leptos_styling::style_sheet!(style, "src/page.css", "app");
view! { <main class=style::PAGE /> }
```
