# Oversized view attribute groups

## Proposition

Add an `oversized_view_attribute_groups` lint for named attribute groups that still contain too many
or too varied direct attributes to represent one coherent element responsibility.

```rust
// Bad: a broad heading legitimizes the complete element interface.
// Button configuration
id=id
type="submit"
form=form_id
aria-label=label
aria-busy=pending
disabled=pending
class="primary"
class:pending=pending
style:width=width
data-source=source
on:click=submit
node_ref=button_ref
```

```rust
// Better: bounded semantic groups.
// Submission identity
id=id
type="submit"
form=form_id

// Availability and progress
aria-label=label
aria-busy=pending
disabled=pending

// Visual treatment
class="primary"
class:pending=pending
style:width=width

// Submission integration
data-source=source
on:click=submit
node_ref=button_ref
```

An even stronger result may be extraction:

```rust
<SubmitButton
    form=form_id
    state=submission_state
    on_submit=submit
/>
```

## Rationale

As with view sections and function phases, comments must not permit unlimited complexity. A broad
attribute heading can be generated mechanically and leave the original scanning problem unchanged.
A maximum group size forces comments to describe bounded responsibilities and eventually pressures
the interface toward named types or components.

## Conservative detection

- Start groups at comments recognized by `missing_view_attribute_group_comments` analysis.
- Count direct attributes and a weighted category/reactivity complexity score.
- Diagnose groups above configurable `max_view_attribute_group_complexity`.
- Increase weight for event handlers, spreads, multi-line closures, and reactive expressions.
- Avoid counting closure body complexity twice when core function-phase analysis owns it.
- Report the complete oversized group once.

## Extraction guidance

For native elements, several groups may be legitimate because HTML exposes separate platform
concerns. For custom components, multiple oversized groups more strongly indicate an oversized prop
interface. Recommendations may include:

- a domain options or state type;
- typed events instead of several callbacks;
- a narrower child component;
- a semantic wrapper around a recurring native-element contract.

The lint should not recommend adding a DOM wrapper solely to reduce source complexity.

## Relationship with other rules

- `missing_view_attribute_group_comments` establishes boundaries.
- `mismatched_view_attribute_groups` checks semantic cohesion.
- `markup_repeating_view_comments` rejects headings that merely restate attribute names.
- `boolean_component_props`, `writable_signal_component_props`, and
  `implicit_default_component_props` may independently reveal why a custom component interface is
  difficult to group.

## Open decisions

- Whether count and complexity limits differ between native elements and components.
- Whether one multi-line event closure belongs entirely to core code-phase analysis.
- Whether a group crossing many categories is oversized even below the numerical limit.
- Whether repeated group shapes across call sites should propose a new component automatically.
- Whether a single spread makes the group's true size too uncertain for a deterministic finding.
