# leptos_styling_untyped_component_classes

## What it does

Requires Leptos class values to be composed from generated constants belonging to the paired local
stylesheet.

## Why is this bad?

Raw class strings, class directives, foreign constants, and opaque class helpers bypass Turf's class
rewriting and defeat local ownership checks.

## Example

```rust
view! { <button class="primary">"Save"</button> }
```

## Use instead

```rust
view! { <button class=style::PRIMARY>"Save"</button> }
```
