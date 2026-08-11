# Unstable For keys

## Proposition

Add an `unstable_for_keys` lint for Leptos `For` and keyed-store views whose key expressions do not
represent stable, unique item identity.

```rust
// Bad: every row has the same identity.
<For each=move || users.get() key=|_| 0 children=render_user/>

// Bad for reorderable collections: position is not item identity.
<For
    each=move || users.get().into_iter().enumerate()
    key=|(index, _)| *index
    children=render_user
/>
```

```rust
// Better: use stable domain identity.
<For
    each=move || users.get()
    key=|user| user.id
    children=render_user
/>
```

Leptos uses keys to retain the relationship between data items and rendered rows. Its iteration
guide documents correctness problems when index-based access is retained across collection
reordering.

Reference: [Iterating over More Complex Data](https://book.leptos.dev/view/04b_iteration.html).

## Conservative detection

- Inspect `key` props on `For`, `ForEnumerate`, keyed stores, and configured wrappers.
- Diagnose constant, unit, boolean, and obvious index-only keys.
- Diagnose enumeration indices used as keys when the collection is reactively inserted, removed,
  sorted, or reordered.
- Increase confidence when the item type exposes a field named `id`, `key`, or another configured
  identity field that is ignored.
- Detect keys containing the whole mutable row or presentation value, which force replacement on
  ordinary edits.
- Report the identity problem once at the key expression.

## Append-only and static collections

An index can be stable for a provably immutable or append-only sequence. A plain iterator rendered
once may not need keyed reactivity at all. The lint should distinguish:

- static iteration;
- append-only logs;
- mutable reorderable collections;
- `ForEnumerate`, where index is exposed reactively but should not become identity accidentally.

## Diagnostic direction

Recommend a stable domain identifier, a keyed store field, or a different iteration primitive. Do
not mechanically choose a field merely because it implements `Eq` and `Hash`; uniqueness and
stability are semantic properties.

## Open decisions

- Whether index keys are always forbidden or permitted for proven append-only data.
- How mutation behavior of the source collection is inferred across helpers.
- Whether cloning a large item as its key receives a performance diagnostic here.
- Whether key expressions involving mutable values should warn even when combined with an ID.
- How custom keyed components declare their source and key props.
