# leptos_unnamed_composables

## What it does

Requires project-local helpers that own reactive behavior and are consumed by components or composables to start with `use_`.

## Why is this bad?

Hidden reactive ownership makes lifecycle behavior surprising at the call site.

## Example

```rust,ignore
fn editor_state() -> RwSignal<EditorDraft> { RwSignal::new(EditorDraft::default()) }
```

## Use instead

```rust,ignore
fn use_editor_state() -> RwSignal<EditorDraft> { RwSignal::new(EditorDraft::default()) }
```
