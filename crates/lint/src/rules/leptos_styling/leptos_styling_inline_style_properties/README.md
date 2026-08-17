# leptos_styling_inline_style_properties

## What it does

Rejects direct inline CSS properties in Leptos views while allowing dynamic values to cross into the
paired stylesheet through custom properties.

## Why is this bad?

Inline presentation splits a component's visual policy between Rust markup and CSS and cannot be
validated as part of the colocated stylesheet.

## Example

```rust
view! { <div style=format!("width: {percent}%") /> }
```

## Use instead

```rust
view! { <div style=("--progress", move || format!("{percent}%")) /> }
```
