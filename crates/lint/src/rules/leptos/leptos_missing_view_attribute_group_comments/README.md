# leptos_missing_view_attribute_group_comments

## What it does

Finds dense Leptos opening tags whose attributes span several responsibilities without named
groups.

## Why is this bad?

A large opening tag is a compact interface definition. When identity, state, accessibility,
presentation, and behavior are interleaved, readers must repeatedly classify every binding before
they can understand the element's contract.

## Example

```rust,ignore
<button type="submit" form=form_id class="primary" class:pending=pending
    disabled=pending aria-busy=pending on:click=submit>
    "Save"
</button>
```

## Use instead

Name stable, element-specific responsibilities, or extract a narrower component:

```rust,ignore
<button
    // Submission identity
    type="submit" form=form_id

    // Availability and progress
    disabled=pending aria-busy=pending

    // Submission presentation and behavior
    class="primary" class:pending=pending on:click=submit
>
    "Save"
</button>
```

## Configuration

`leptos-unnamed-view-attribute-complexity-threshold` sets when attributes need a group heading (default `6`).
