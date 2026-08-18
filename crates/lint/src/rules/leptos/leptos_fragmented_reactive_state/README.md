# leptos_fragmented_reactive_state

## What it does

Limits the reactive primitives created directly by each component and composable.

## Why is this bad?

Many independent signals hide cohesive state transitions and permit invalid intermediate combinations.

## Example

```rust,ignore
let name = RwSignal::new(String::new());
let email = RwSignal::new(String::new());
let phone = RwSignal::new(String::new());
let city = RwSignal::new(String::new());
let country = RwSignal::new(String::new());
```

## Use instead

```rust,ignore
let draft = RwSignal::new(ContactDraft::default());
```

## Configuration

`leptos-reactive-primitives-threshold` sets the maximum separate reactive primitives (default `4`).
