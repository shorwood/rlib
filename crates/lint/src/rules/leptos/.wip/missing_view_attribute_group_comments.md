# Missing view attribute-group comments

## Proposition

Add a `missing_view_attribute_group_comments` lint for dense native elements or component
invocations whose attributes mix several responsibilities without named visual groups.

```rust
// Bad: identity, state, presentation, accessibility, and behavior are interleaved.
view! {
    <button
        type="submit"
        class="primary-action"
        on:click=submit
        disabled=move || !can_submit.get()
        aria-busy=move || is_submitting.get()
        form=form_id
        class:pending=move || is_submitting.get()
    >
        "Save changes"
    </button>
}
```

```rust
// Better: attribute phases expose the element's contract.
view! {
    <button
        // Submission identity
        type="submit"
        form=form_id

        // Availability and progress
        disabled=move || !can_submit.get()
        aria-busy=move || is_submitting.get()

        // Visual treatment
        class="primary-action"
        class:pending=move || is_submitting.get()

        // Submission behavior
        on:click=submit
    >
        "Save changes"
    </button>
}
```

## Rationale

A dense opening tag is a compact interface definition. Its attributes can simultaneously define
identity, accessibility, state, styling, data flow, behavior, and framework integration. Without
groups, readers must repeatedly classify each token before understanding the element.

Comments should appear only after density makes them useful. Requiring groups on every element
would create generated scaffolding more distracting than the attributes themselves.

## Conservative detection

Possible configurable evidence:

- more than six direct attributes or props;
- three or more semantic categories;
- multiple reactive bindings;
- at least one event handler plus state or presentation bindings;
- a component invocation whose props span independent domain responsibilities.

A weighted score is preferable to a single attribute count because seven static `data-*` values are
different from seven attributes spanning behavior and accessibility.

## Attribute categories

| Category | Examples |
|---|---|
| Identity and semantics | `id`, `name`, `type`, `href`, `action` |
| Accessibility | `role`, `aria:*`, `alt`, label relationships |
| Value and state | `prop:value`, `checked`, `disabled`, `hidden` |
| Presentation | `class`, `class:*`, `style`, `style:*` |
| Data | `data-*` |
| Behavior | `on:*` |
| Integration | `node_ref`, `use:*`, spreads |

Categories provide structural evidence; headings should still name the element-specific purpose.
`Submission behavior` is stronger than the mechanically generated `Events`.

## Components and extraction pressure

```rust
<Editor
    // Document identity
    document_id=document.id
    revision=document.revision

    // Editing permissions
    access=editor_access
    read_only=is_read_only

    // Persistence events
    on_save=save
    on_conflict=resolve_conflict
/>
```

If a component routinely needs many groups, its interface may own too many responsibilities or need
named domain configuration types. The diagnostic should mention extraction rather than treating
comments as the permanent solution.

## Open decisions

- The default score, count, and category thresholds.
- Whether native elements and components use different thresholds.
- Whether coherent category ordering can remove the need for comments in some cases.
- Whether spreads count as one opaque category or increase uncertainty strongly.
- Whether one complex reactive attribute can make an otherwise small tag eligible.
