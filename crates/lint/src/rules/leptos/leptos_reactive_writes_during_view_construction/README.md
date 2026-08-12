# leptos_reactive_writes_during_view_construction

## What it does

Checks for reactive state written directly while a suspended Leptos view is being resolved.
Writes inside event handlers and other callbacks are accepted because they run in response to an
explicit action rather than as part of rendering.

## Why is this bad?

An asynchronous or reactive view may be constructed more than once. Writing state during that
process can overwrite a user's newer changes, schedule another render, or make the result depend
on how often Leptos evaluates the view.

## Example

```rust,ignore
view! {
    <Suspense>
        {move || Suspend::new(async move {
            let items = load_items().await;
            selected.set(initial_selection(&items));
            view! { <ItemList items/> }
        })}
    </Suspense>
}
```

## Use instead

Give a child component ownership of state initialized from the loaded value:

```rust,ignore
view! {
    <Suspense>
        {move || Suspend::new(async move {
            let items = load_items().await;
            view! { <SelectableItemList items/> }
        })}
    </Suspense>
}

#[component]
fn SelectableItemList(items: Vec<Item>) -> impl IntoView {
    let selected = RwSignal::new(initial_selection(&items));
    // ...
}
```
