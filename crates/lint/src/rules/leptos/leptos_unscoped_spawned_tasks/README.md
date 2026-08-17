# leptos_unscoped_spawned_tasks

## What it does

Rejects direct or aliased `spawn_local` calls inside components and composables.

## Why is this bad?

Detached tasks hide ownership, cancellation, loading, and mutation semantics from the reactive graph.

## Example

```rust,ignore
spawn_local(async move { save_contact(draft).await });
```

## Use instead

```rust,ignore
let save = Action::new(|draft: &ContactDraft| save_contact(draft.clone()));
save.dispatch(draft);
```
