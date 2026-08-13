# leptos_mismatched_view_attribute_groups

## What it does

Finds Leptos attributes placed beneath a group heading that explicitly names a contradictory
behavioral category.

## Why is this bad?

An attribute-group heading makes a structural claim. A behavior binding beneath a presentation
heading, for example, turns that claim into misleading decoration.

## Example

```rust,ignore
<button
    // Submission presentation
    class="primary"
    on:click=submit
/>
```

## Use instead

Move the binding beneath a matching responsibility or rename a genuinely cross-cutting group:

```rust,ignore
<button
    // Submission presentation
    class="primary"

    // Submission behavior
    on:click=submit
/>
```
