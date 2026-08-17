# leptos_excessive_component_composition_depth

## What it does

Limits the longest acyclic chain in the crate-local component call graph and reports its root once.

## Why is this bad?

Long pass-through chains scatter one screen across too many files and navigation hops.

## Example

```rust,ignore
#[component]
fn Page() -> impl IntoView { view! { <PageShell /> } }
```

## Use instead

```rust,ignore
#[component]
fn Page() -> impl IntoView { view! { <main><PageContent /></main> } }
```
