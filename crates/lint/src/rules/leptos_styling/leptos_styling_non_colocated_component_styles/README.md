# `rlib::leptos_styling_non_colocated_component_styles`

## Summary

Requires every styled Leptos source module to declare exactly one external `leptos_styling` stylesheet with the same stem and the local alias `style`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires every styled Leptos source module to declare exactly one external `leptos_styling`
stylesheet with the same stem and the local alias `style`.

## Why this matters

Shared, foreign, or inline style sources hide component ownership and make style changes ripple across
unrelated markup.

## Examples

### Triggers the lint

```rust
leptos_styling::style_sheet!(shared, "src/styles/app.css", "app");
view! { <main class=shared::PAGE /> }
```

### Use this instead

```rust
leptos_styling::style_sheet!(style, "src/page.css", "app");
view! { <main class=style::PAGE /> }
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
