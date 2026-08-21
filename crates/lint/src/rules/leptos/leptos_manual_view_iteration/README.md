# `rlib::leptos_manual_view_iteration`

## Summary

Checks for standard iterator mapping adapters in chains completed by Leptos `collect_view()`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for `map`, `filter_map`, `flat_map`, `map_while`, or `scan` in a standard iterator
chain completed by Leptos `collect_view()`. Static, tracked, and explicitly untracked collection
sources are handled uniformly.

## Why this matters

Iterator-driven rendering buries repeated UI structure inside a Rust expression. Leptos `<For>`
makes the collection source, identity key, and child template visible together in the view tree, so
the repeated structure is easier to identify and read.

## Examples

### Triggers the lint

```rust,ignore
{USERS
    .iter()
    .map(|user| view! { <UserRow user /> })
    .collect_view()}
```

### Use this instead

Use `<For>` and choose a key that remains stable for the lifetime of the domain item:

```rust,ignore
<For
    each=move || USERS.iter()
    key=|user| user.id
    children=move |user| view! { <UserRow user /> }
/>
```

## What it skips

Collection through `collect::<Vec<_>>()`, helper-mediated iterator construction, and iterators of
already-produced views without a mapping adapter are accepted.

## When to turn it off

Turn this lint off when an iterator pipeline communicates a specialized rendering transformation
more clearly than a declarative `<For>` component.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint cannot infer a stable domain key or mechanically separate filtering and mapping logic, so
migration to `<For>` is manual.

## Related lints

- [`rlib::leptos_unstable_for_keys`](../leptos_unstable_for_keys/README.md) — Checks that the
  replacement `<For>` key represents stable item identity.
