# `rlib::leptos_styling_untyped_component_classes`

## Summary

Requires Leptos class values to be composed from generated constants belonging to the paired local stylesheet.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos_styling` |
| Cargo feature | `leptos_styling` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires Leptos class values to be composed from generated constants belonging to the paired local
stylesheet.

## Why this matters

Raw class strings, class directives, foreign constants, and opaque class helpers bypass Turf's class
rewriting and defeat local ownership checks.

## Examples

### Triggers the lint

```rust
view! { <button class="primary">"Save"</button> }
```

### Use this instead

```rust
view! { <button class=style::PRIMARY>"Save"</button> }
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
