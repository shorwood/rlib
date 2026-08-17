# leptos_oversized_event_handlers

## What it does

Limits statement count and nested control flow in `on:*` handlers and `Callback::new` closures.

## Why is this bad?

Complex inline handlers conceal intent and leave application behavior difficult to test independently.

## Example

```rust,ignore
view! { <button on:click=move |_| { validate(); save(); refresh(); close(); }>"Save"</button> }
```

## Use instead

```rust,ignore
let save = use_save_contact();
view! { <button on:click=move |_| save.dispatch(())>"Save"</button> }
```
