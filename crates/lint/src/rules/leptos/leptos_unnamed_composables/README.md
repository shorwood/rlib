# `rlib::leptos_unnamed_composables`

## Summary

Requires project-local helpers that own reactive behavior and are consumed by components or composables to start with `use_`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires project-local helpers that own reactive behavior and are consumed by components or composables to start with `use_`.

## Why this matters

Hidden reactive ownership makes lifecycle behavior surprising at the call site.

## Examples

### Triggers the lint

```rust,ignore
fn editor_state() -> RwSignal<EditorDraft> { RwSignal::new(EditorDraft::default()) }
```

### Use this instead

```rust,ignore
fn use_editor_state() -> RwSignal<EditorDraft> { RwSignal::new(EditorDraft::default()) }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
