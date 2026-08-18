# `rlib::leptos_unscoped_spawned_tasks`

## Summary

Warns about direct or aliased `spawn_local` calls inside components and composables.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about direct or aliased `spawn_local` calls inside components and composables.

## Why this matters

Detached tasks hide ownership, cancellation, loading, and mutation semantics from the reactive graph.

## Examples

### Triggers the lint

```rust,ignore
spawn_local(async move { save_contact(draft).await });
```

### Use this instead

```rust,ignore
let save = Action::new(|draft: &ContactDraft| save_contact(draft.clone()));
save.dispatch(draft);
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
