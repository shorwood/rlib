# leptos_excessively_nested_views

## What it does

Limits static tag nesting and embedded Rust control-flow nesting within an authored component view.

## Why is this bad?

Deep views obscure visual hierarchy, component states, and accessible structure.

## Example

```rust,ignore
view! { <main><section><div><div><div><div><div><div>"value"</div></div></div></div></div></div></section></main> }
```

## Use instead

```rust,ignore
view! { <main><SummarySection /></main> }
```
