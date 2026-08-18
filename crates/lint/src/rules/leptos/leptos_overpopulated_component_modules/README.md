# leptos_overpopulated_component_modules

## What it does

Limits authored component and island definitions per source or inline module.

## Why is this bad?

Crowded modules blur ownership and make unrelated UI responsibilities change together.

## Example

```rust,ignore
#[component] fn A() -> impl IntoView { view! { <div /> } }
#[component] fn B() -> impl IntoView { view! { <div /> } }
#[component] fn C() -> impl IntoView { view! { <div /> } }
#[component] fn D() -> impl IntoView { view! { <div /> } }
#[component] fn E() -> impl IntoView { view! { <div /> } }
```

## Use instead

```rust,ignore
mod editor;
mod summary;
```

## Configuration

`leptos-components-per-module-threshold` sets the maximum components per module (default `8`).
