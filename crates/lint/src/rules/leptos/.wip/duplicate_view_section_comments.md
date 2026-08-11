# Duplicate view section comments

## Proposition

Add a `duplicate_view_section_comments` lint for two sections at the same direct sibling depth that
use the same normalized heading.

```rust
// Bad: the repeated heading conceals the actual distinction.
view! {
    <main>
        // Account actions
        <EditAccount/>

        // Account actions
        <DeleteAccount/>
    </main>
}
```

```rust
// Better
view! {
    <main>
        // Profile editing
        <EditAccount/>

        // Destructive account actions
        <DeleteAccount/>
    </main>
}
```

## Rationale

A section heading claims that its nodes share one responsibility. Repeating the same heading in one
sibling scope either splits one region arbitrarily or proves that the heading is too broad to name
the distinction. In both cases, the source map is misleading.

This rule deliberately steers authors back toward naming. It must not suggest adding numeric
suffixes or synonyms merely to obtain uniqueness.

## Scope and normalization

- Uniqueness is enforced within one direct sibling list.
- Nested child scopes have independent heading namespaces.
- Separate component functions may reuse the same heading.
- Comparison uses canonicalized sentence content and protected acronym handling.
- Cosmetic differences in case or punctuation do not make headings unique.
- Singular/plural and close semantic variants may provide lower-confidence evidence for a future
  incoherence rule, but exact normalized duplicates are the initial deterministic target.

## Conservative detection

- Consume valid headings identified by `malformed_view_section_comments` analysis.
- Record the first heading span at each direct tree depth.
- Diagnose later duplicates and relate them to the original location.
- Include the direct component or element names owned by both sections when available.
- Avoid a machine suggestion because the correct distinction is semantic.

## Diagnostic direction

> `Account actions` already names an earlier section in this view. Merge the regions if they share
> one responsibility, or rename them according to the distinct behavior they own; do not split the
> same concept into arbitrary groups.

## Open decisions

- Whether nonadjacent repetitions separated by a major control-flow boundary remain duplicates.
- Whether headings such as `Account action` and `Account actions` are normalized together.
- Whether common generic headings receive a stronger diagnostic even when used once.
- Whether corresponding sections in separate conditional branches may intentionally share a name.
- How duplicated headings inside macro repetitions are reported without diagnostic noise.
