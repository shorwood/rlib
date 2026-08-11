# Oversized view sections

## Proposition

Add an `oversized_view_sections` lint for named view regions whose direct complexity still exceeds a
configurable bound.

```rust
// Bad: one broad heading legitimizes an entire page.
// Account management
<AccountHeader/>
<AccountProfile/>
<AccountEmail/>
<AccountPassword/>
<AccountSessions/>
<AccountTokens/>
<AccountAuditLog/>
<AccountDeletion/>
```

```rust
// Better: the source exposes real responsibilities.
// Account identity
<AccountHeader/>
<AccountProfile/>
<AccountEmail/>

// Account security
<AccountPassword/>
<AccountSessions/>
<AccountTokens/>
<AccountAuditLog/>

// Account destruction
<AccountDeletion/>
```

The stronger result may be component extraction:

```rust
<AccountIdentity/>
<AccountSecurity/>
<AccountDestruction/>
```

## Rationale

Without a maximum, section comments become permission to retain arbitrarily large views. Generated
code can satisfy a missing-comment rule by inserting generic headings every few lines without
improving structure. The section bound ensures that headings reveal cohesion and eventually force
the code to discover stable component boundaries.

## Conservative detection

- Reuse the direct complexity score from `missing_view_section_comments`.
- Start a section at each valid direct-child heading.
- Count only direct constructs owned by that sibling scope.
- Diagnose any section above `max_view_section_complexity`, including the first section.
- Treat a single highly complex child as a possible extraction problem without double-counting its
  descendants.
- Report the complete oversized region once.

## Naming as evidence

A heading unable to summarize its contents concisely is evidence that the section is not cohesive.
The diagnostic should ask authors to reconsider both the grouping and the names:

> The section `Account management` contains identity, security, audit, and destruction behavior.
> Split it along stable responsibilities or extract named components; adding more generic headings
> will not make the ownership clearer.

Automated semantic classification can use component names as evidence but should avoid pretending
to know the correct product architecture.

## When comments are enough

Comments are appropriate for small fragments whose nodes need to remain siblings because extra DOM
containers would alter layout or semantics. Extraction is preferable when a region has its own
props, reactive state, events, loading behavior, or reusable vocabulary.

## Open decisions

- Whether the complexity limit matches or differs from the initial-comment threshold.
- Whether one control-flow component can exceed the limit based on its direct attributes and props.
- Whether repeated section shapes suggest component extraction strongly enough for another lint.
- Whether a heading above one already well-named component is always redundant.
- How diagnostics distinguish a source component boundary from an unwanted DOM wrapper.
