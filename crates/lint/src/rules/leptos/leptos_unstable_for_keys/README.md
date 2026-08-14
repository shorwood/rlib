# leptos_unstable_for_keys

## What it does

Checks parsed `<For>` keys that do not derive from the row or that return the position of an
enumerated collection. Identifier spelling and whitespace do not determine the result.

## Why is this bad?

Leptos uses keys to keep rendered rows attached to their data. A repeated key cannot distinguish
rows, while a position changes when items are inserted, removed, or reordered. Either choice can
preserve the wrong row state.

## Example

```rust,ignore
<For
    each=move || users.get().into_iter().enumerate()
    key=|(index, _)| *index
    children=render_user
/>
```

## Use instead

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
