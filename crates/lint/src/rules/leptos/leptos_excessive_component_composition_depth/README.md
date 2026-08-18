# `rlib::leptos_excessive_component_composition_depth`

## Summary

Limits the longest acyclic chain in the crate-local component call graph and reports its root once.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits the longest acyclic chain in the crate-local component call graph and reports its root once.

## Why this matters

Long pass-through chains scatter one screen across too many files and navigation hops.

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn Page() -> impl IntoView { view! { <PageShell /> } }
```

### Use this instead

```rust,ignore
#[component]
fn Page() -> impl IntoView { view! { <main><PageContent /></main> } }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-component-composition-depth-threshold` | positive integer | `10` | Sets the deepest allowed chain of locally composed components. |

## Known limitations

No known implementation limitations.

## Related lints

None.
