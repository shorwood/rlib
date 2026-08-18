# `rlib::leptos_styling_unused_stylesheet_classes`

## Summary

Finds class selectors in a paired component stylesheet that have no corresponding typed reference in the owning Rust module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds class selectors in a paired component stylesheet that have no corresponding typed reference in
the owning Rust module.

## Why this matters

Unused selectors accumulate dead presentation policy and often conceal markup that was only partly
removed or renamed.

## Examples

### Triggers the lint

```rust
// page.css also declares `.forgotten`, but this module never references style::FORGOTTEN.
view! { <main class=style::PAGE /> }
```

### Use this instead

```rust
// Remove `.forgotten` from page.css.
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
