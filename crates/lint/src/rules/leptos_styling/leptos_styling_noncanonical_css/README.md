# `rlib::leptos_styling_noncanonical_css`

## Summary

Strictly parses registered CSS files and requires their source to match deterministic Malva output.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Strictly parses registered CSS files and requires their source to match deterministic Malva output.

## Why this matters

Malformed or inconsistently rendered CSS is harder to review and makes formatting noise obscure
behavioral style changes.

## Examples

### Triggers the lint

```rust
// page.css: .page{display:grid;color:red}
leptos_styling::style_sheet!(style, "src/page.css", "app");
```

### Use this instead

```rust
// page.css is formatted by Malva before this declaration is checked.
leptos_styling::style_sheet!(style, "src/page.css", "app");
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-css-max-width` | positive integer | `100` | Sets the line width used to format component CSS. |

## Known limitations

No known implementation limitations.

## Related lints

None.
