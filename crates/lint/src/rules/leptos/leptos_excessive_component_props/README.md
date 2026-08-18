# `rlib::leptos_excessive_component_props`

## Summary

Limits component props while excluding Leptos `Children*` composition props.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits component props while excluding Leptos `Children*` composition props.

## Why this matters

Components with many props are harder to call, test, and change.

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn Editor(a: String, b: String, c: String, d: String, e: String, f: String, g: String) -> impl IntoView { view! { <div /> } }
```

### Use this instead

```rust,ignore
#[component]
fn Editor(draft: EditorDraft, actions: EditorActions) -> impl IntoView { view! { <div /> } }
```

## What it skips

It does not count Leptos `Children*` composition props.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-component-props-threshold` | positive integer | `6` | Sets the largest allowed number of component props, excluding `Children*` props. |

## Known limitations

No known implementation limitations.

## Related lints

None.
