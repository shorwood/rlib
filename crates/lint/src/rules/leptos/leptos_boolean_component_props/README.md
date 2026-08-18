# `rlib::leptos_boolean_component_props`

## Summary

Warns about direct, optional, and reactive boolean properties on Leptos components when their names do not communicate a standard binary state.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about direct, optional, and reactive boolean properties on Leptos components when their
names do not communicate a standard binary state. The platform states `disabled` and
`invalid` are accepted because HTML and ARIA already define their meaning and conventional
opposite. Booleans nested inside callback signatures or unknown application wrappers are
not treated as component state by this rule.

## Why this matters

Component calls read as declarative markup. A boolean value does not name the selected
state at the call site, makes the opposite state implicit, and cannot grow to represent
another state without changing the property type. A domain enum keeps the component's
names visible in both its declaration and its uses. This does not apply to conventional
platform states such as `disabled`: replacing that familiar two-state behavior with an enum
would obscure interoperability rather than clarify domain names.

For example, presentation flags leave the selected design variant implicit:

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn Badge(compact: bool, featured: bool) -> impl IntoView {
    // ...
}
```

### Use this instead

Prefer a domain type that names every supported presentation:

```rust,ignore
enum BadgeDensity { Comfortable, Compact }
enum BadgeEmphasis { Standard, Featured }

#[component]
fn Badge(density: BadgeDensity, emphasis: BadgeEmphasis) -> impl IntoView {
    // ...
}
```

## What it skips

Warns about direct, optional, and reactive boolean properties on Leptos components when their names do not communicate a standard binary state. Booleans nested inside callback signatures or unknown application wrappers are not treated as component state by this rule.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
