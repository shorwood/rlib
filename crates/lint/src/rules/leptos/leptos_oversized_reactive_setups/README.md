# leptos_oversized_reactive_setups

## What it does

Limits top-level setup statements in components and `use_*` composables, excluding the returned tail expression.

## Why is this bad?

Large setup phases mix independent reactive responsibilities and make component lifecycles difficult to follow.

## Example

```rust,ignore
#[component]
fn Editor() -> impl IntoView {
    let a = signal(0); let b = signal(0); let c = signal(0); let d = signal(0);
    let e = signal(0); let f = signal(0); let g = signal(0); let h = signal(0); let i = signal(0);
    view! { <div /> }
}
```

## Use instead

```rust,ignore
#[component]
fn Editor() -> impl IntoView {
    let form = use_editor_form();
    view! { <EditorForm form /> }
}
```
