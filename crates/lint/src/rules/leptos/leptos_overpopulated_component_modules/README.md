# `rlib::leptos_overpopulated_component_modules`

## Summary

Limits hand-written component and island definitions per source or inline module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits hand-written component and island definitions per source or inline module.

## Why this matters

Crowded modules blur ownership and make unrelated UI responsibilities change together.

## Examples

### Triggers the lint

```rust,ignore
#[component] fn A() -> impl IntoView { view! { <div /> } }
#[component] fn B() -> impl IntoView { view! { <div /> } }
#[component] fn C() -> impl IntoView { view! { <div /> } }
#[component] fn D() -> impl IntoView { view! { <div /> } }
#[component] fn E() -> impl IntoView { view! { <div /> } }
```

### Use this instead

```rust,ignore
mod editor;
mod summary;
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-components-per-module-threshold` | positive integer | `8` | Sets how many components may be declared in one module. |

## Known limitations

No known implementation limitations.

## Related lints

None.
