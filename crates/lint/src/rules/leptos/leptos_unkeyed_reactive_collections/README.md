# `rlib::leptos_unkeyed_reactive_collections`

## Summary

Checks for a tracked reactive collection read, transformed into views by a standard iterator mapping adapter, and finished with `collect_view()`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for a tracked reactive collection read, transformed into views by a standard iterator
mapping adapter, and finished with `collect_view()`. Static and explicitly untracked reads are
accepted.

## Why this matters

Collected child views are associated with their position. When a reactive collection is filtered,
reordered, or receives an insertion, positional identity can recreate DOM nodes and lose local
element state such as focus or input selection.

## Examples

### Triggers the lint

```rust,ignore
{move || users.get()
    .into_iter()
    .map(|user| view! { <UserRow user/> })
    .collect_view()}
```

### Use this instead

Use `<For>` and choose a key that remains stable for the lifetime of the domain item:

```rust,ignore
<For
    each=move || users.get()
    key=|user| user.id
    children=move |user| view! { <UserRow user/> }
/>
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
