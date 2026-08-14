# leptos_unkeyed_reactive_collections

## What it does

Checks for a tracked reactive collection read, transformed into views by a standard iterator
mapping adapter, and finished with `collect_view()`. Static and explicitly untracked reads are
accepted.

## Why is this bad?

Collected child views are associated with their position. When a reactive collection is filtered,
reordered, or receives an insertion, positional identity can recreate DOM nodes and lose local
element state such as focus or input selection.

## Example

```rust,ignore
{move || users.get()
    .into_iter()
    .map(|user| view! { <UserRow user/> })
    .collect_view()}
```

## Use instead

Use `<For>` and choose a key that remains stable for the lifetime of the domain item:

```rust,ignore
<For
    each=move || users.get()
    key=|user| user.id
    children=move |user| view! { <UserRow user/> }
/>
```
