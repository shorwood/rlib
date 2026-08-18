# `rlib::leptos_styling_inline_style_properties`

## Summary

Warns about direct inline CSS properties in Leptos views while allowing dynamic values to cross into the paired stylesheet through custom properties.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about direct inline CSS properties in Leptos views while allowing dynamic values to cross into the
paired stylesheet through custom properties.

## Why this matters

Inline presentation splits a component's visual policy between Rust markup and CSS and cannot be
validated as part of the colocated stylesheet.

## Examples

### Triggers the lint

```rust
view! { <div style=format!("width: {percent}%") /> }
```

### Use this instead

```rust
view! { <div style=("--progress", move || format!("{percent}%")) /> }
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
