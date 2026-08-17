# leptos_styling_unscoped_component_selectors

## What it does

Requires every ordinary selector branch in a paired component stylesheet to be anchored by a class
from that stylesheet and forbids stylesheet imports.

## Why is this bad?

Classless or imported rules let a component mutate unrelated document state and recreate a hidden
global stylesheet.

## Example

```rust
// page.css: button { color: red; }
view! { <button class=style::BUTTON>"Delete"</button> }
```

## Use instead

```rust
// page.css: .button { color: red; }
view! { <button class=style::BUTTON>"Delete"</button> }
```
