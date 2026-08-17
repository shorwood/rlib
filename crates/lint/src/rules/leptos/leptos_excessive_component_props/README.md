# leptos_excessive_component_props

## What it does

Limits component props while excluding Leptos `Children*` composition props.

## Why is this bad?

Large prop surfaces expose internal coordination and make component call sites difficult to understand.

## Example

```rust,ignore
#[component]
fn Editor(a: String, b: String, c: String, d: String, e: String, f: String, g: String) -> impl IntoView { view! { <div /> } }
```

## Use instead

```rust,ignore
#[component]
fn Editor(draft: EditorDraft, actions: EditorActions) -> impl IntoView { view! { <div /> } }
```
