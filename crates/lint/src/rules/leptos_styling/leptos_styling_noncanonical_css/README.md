# leptos_styling_noncanonical_css

## What it does

Strictly parses registered CSS files and requires their source to match deterministic Malva output.

## Why is this bad?

Malformed or inconsistently rendered CSS is harder to review and makes formatting noise obscure
behavioral style changes.

## Example

```rust
// page.css: .page{display:grid;color:red}
leptos_styling::style_sheet!(style, "src/page.css", "app");
```

## Use instead

```rust
// page.css is formatted by Malva before this declaration is checked.
leptos_styling::style_sheet!(style, "src/page.css", "app");
```
