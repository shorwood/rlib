# leptos_writable_signal_component_props

## What it does

Rejects direct and optional component properties proven to implement Leptos's reactive `Write`,
`Set`, `Update`, or `UpdateUntracked` capabilities. A prop used exclusively by one native `bind:*`
is accepted as a transparent control boundary.

## Why is this bad?

Writable reactive handles let descendants perform arbitrary state transitions owned by an
ancestor. Read-only state plus intent-bearing callbacks keeps mutation ownership explicit,
makes validation and instrumentation discoverable, and narrows the child component's API.

## Example

```rust
#[component]
fn DeleteButton(selected: RwSignal<Option<UserId>>) -> impl IntoView {
    view! { <button on:click=move |_| selected.set(None)>"Delete"</button> }
}
```

## Use instead

```rust
#[component]
fn DeleteButton(
    selected: Signal<Option<UserId>>,
    on_delete: Callback<UserId>,
) -> impl IntoView {
    view! { <button on:click=move |_| selected.get().map(|id| on_delete.run(id))>"Delete"</button> }
}
```
