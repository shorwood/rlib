# leptos_styling_unused_stylesheet_classes

## What it does

Finds class selectors in a paired component stylesheet that have no corresponding typed reference in
the owning Rust module.

## Why is this bad?

Unused selectors accumulate dead presentation policy and often conceal markup that was only partly
removed or renamed.

## Example

```rust
// page.css also declares `.forgotten`, but this module never references style::FORGOTTEN.
view! { <main class=style::PAGE /> }
```

## Use instead

```rust
// Remove `.forgotten` from page.css.
view! { <main class=style::PAGE /> }
```
