# Mismatched view attribute groups

## Proposition

Add a `mismatched_view_attribute_groups` lint for attributes or component props placed beneath a
group heading that does not coherently describe them.

```rust
// Bad: one generic heading contains three unrelated categories.
// Accessibility
aria-label=label
class="primary"
on:click=submit
```

```rust
// Better
// Accessible submission state
aria-label=label
aria-busy=is_submitting
disabled=is_submitting

// Submission presentation
class="primary"
class:pending=is_submitting

// Submission behavior
on:click=submit
```

## Rationale

An attribute-group comment makes a semantic claim about the props that follow it. If unrelated
attributes accumulate beneath that heading, the comment becomes decorative rather than structural.
The lint should protect the integrity of the visual language established by group comments.

This rule is analogous to section-divider coherence: a heading is only valuable when the owned items
support its stated responsibility.

## Conservative detection

- Parse valid attribute-group headings and the attributes they own until the next heading.
- Classify known native attributes into identity, accessibility, state, presentation, data,
  behavior, and integration categories.
- Use component prop names, callback types, reactive types, and documentation as weaker evidence for
  custom components.
- Diagnose high-confidence category contradictions, such as `on:*` under presentation or `style:*`
  under accessibility.
- Diagnose groups with several unrelated categories and no semantic relationship in the heading.
- Avoid claiming that closely related cross-category attributes are incoherent.

## Semantic groups may cross categories

```rust
// Availability and progress
disabled=is_submitting
aria-busy=is_submitting
class:pending=is_submitting
```

This group crosses state, accessibility, and presentation but has one shared semantic cause. A rigid
category-order lint would incorrectly split it. Dataflow and shared naming should therefore outweigh
mechanical categories when they prove cohesion.

## Diagnostic direction

> `Submission presentation` also owns the `on:click` handler, which defines behavior rather than
> presentation. Move it beneath a behavior heading, rename the group to reflect one shared
> responsibility, or extract the interaction.

No automatic move should be offered when closures, comments, or spreads make source boundaries
uncertain.

## Open decisions

- Whether fixed category vocabulary is sufficient for native elements.
- How shared reactive dependencies establish a cross-category semantic group.
- Whether component prop documentation can supply reliable category evidence.
- Whether generic headings are malformed before mismatch analysis runs.
- Whether ordering within one coherent group should be alphabetical, semantic, or unconstrained.
