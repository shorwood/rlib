# `rlib::leptos_oversized_reactive_setups`

## Summary

Limits top-level setup statements in components and `use_*` composables, excluding the returned tail expression.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits top-level setup statements in components and `use_*` composables, excluding the returned tail expression.

## Why this matters

Large setup phases mix independent reactive responsibilities and make component lifecycles difficult to follow.

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn Editor() -> impl IntoView {
    let a = signal(0); let b = signal(0); let c = signal(0); let d = signal(0);
    let e = signal(0); let f = signal(0); let g = signal(0); let h = signal(0); let i = signal(0);
    view! { <div /> }
}
```

### Use this instead

```rust,ignore
#[component]
fn Editor() -> impl IntoView {
    let form = use_editor_form();
    view! { <EditorForm form /> }
}
```

## What it skips

Limits top-level setup statements in components and `use_*` composables, excluding the returned tail expression.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-setup-statements-threshold` | positive integer | `8` | Sets how many setup statements may appear before the component view. |

## Known limitations

No known implementation limitations.

## Related lints

None.
