# `rlib::leptos_unstable_for_keys`

## Summary

Checks parsed `<For>` keys that do not derive from the row or that return the position of an enumerated collection.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks parsed `<For>` keys that do not derive from the row or that return the position of an
enumerated collection. Identifier spelling and whitespace do not determine the result.

## Why this matters

Leptos uses keys to keep rendered rows attached to their data. A repeated key cannot distinguish
rows, while a position changes when items are inserted, removed, or reordered. Either choice can
preserve the wrong row state.

## Examples

### Triggers the lint

```rust,ignore
<For
    each=move || users.get().into_iter().enumerate()
    key=|(index, _)| *index
    children=render_user
/>
```

### Use this instead

Choose an identifier that belongs to the item and remains stable while it is displayed:

```rust,ignore
<For
    each=move || users.get()
    key=|user| user.id
    children=render_user
/>
```

An index is suitable only when the sequence is truly fixed or append-only. Such a sequence may
also be clearer as ordinary, non-reactive iteration.

## What it skips

Checks parsed `<For>` keys that do not derive from the row or that return the position of an enumerated collection. Identifier spelling and whitespace do not determine the result.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
