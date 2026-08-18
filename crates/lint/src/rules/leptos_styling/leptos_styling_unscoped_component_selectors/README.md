# `rlib::leptos_styling_unscoped_component_selectors`

## Summary

Requires every ordinary selector branch in a paired component stylesheet to be anchored by a class from that stylesheet and forbids stylesheet imports.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires every ordinary selector branch in a paired component stylesheet to be anchored by a class
from that stylesheet and forbids stylesheet imports.

## Why this matters

Classless or imported rules let a component mutate unrelated document state and recreate a hidden
global stylesheet.

## Examples

### Triggers the lint

```rust
// page.css: button { color: red; }
view! { <button class=style::BUTTON>"Delete"</button> }
```

### Use this instead

```rust
// page.css: .button { color: red; }
view! { <button class=style::BUTTON>"Delete"</button> }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
