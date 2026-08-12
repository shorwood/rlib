# leptos_oversized_view_attribute_groups

## What it does

Finds named Leptos attribute groups whose direct complexity exceeds the configured limit.

## Why is this bad?

A broad heading can legitimize an entire opening-tag interface while leaving the original scanning
problem unchanged. Bounded groups keep comments meaningful and expose extraction pressure.

## Example

```rust,ignore
<button
    // Button configuration
    id=id type="submit" form=form_id class="primary" disabled=pending on:click=submit
/>
```

## Use instead

Split stable responsibilities into focused groups, or extract a narrower component:

```rust,ignore
<button
    // Submission identity
    id=id type="submit" form=form_id

    // Availability and behavior
    disabled=pending on:click=submit
/>
```
