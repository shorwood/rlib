# Malformed view section comments

## Proposition

Add a `malformed_view_section_comments` lint that enforces one canonical syntax, casing style, and
placement for comments used as Leptos view-section headings.

```rust
// Bad
// ACCOUNT MANAGEMENT
// account management
// Account Management:
// stuff
```

```rust
// Good
// Account management
// Active sessions and access tokens
// Destructive account actions
```

## Rationale

Section comments form a visual language. Inconsistent capitalization, decoration, punctuation, and
placement make readers distinguish formatting styles instead of following the view structure. A
strict canonical form also lets the other view-section lints identify boundaries reliably.

The content must remain semantic. Canonical syntax should not make generic or markup-repeating
headings acceptable.

## Canonical syntax

The default form should be a single Rust line comment with sentence-style content:

```rust
// Account recovery actions
```

Possible requirements:

- exactly the configured comment prefix;
- non-empty content;
- sentence case rather than title case or shouting capitals;
- no trailing colon, period, or decorative punctuation;
- no divider glyphs or repeated hyphens;
- one logical line unless a separately configured template permits more;
- protected acronym and product-name handling shared with core comment analysis.

## Placement

A heading must immediately precede the first direct node it owns, allowing indentation and at most
the configured whitespace shape:

```rust
view! {
    // Account security
    <PasswordSettings/>
    <ActiveSessions/>
}
```

It must not appear after the node, inside an unrelated expression, detached by a local declaration,
or at a different tree depth. Blank lines are presentation only and do not change section ownership.

## Conservative detection

- Parse comments from the authored source surrounding `view!` token trees.
- Recognize only comments occupying direct-child boundary positions as section candidates.
- Validate syntax using the shared sentence-case and protected-term utilities.
- Offer a machine-applicable replacement only when normalization is unambiguous.
- Avoid rewriting content whose semantic wording requires judgment.

## Relationship with other rules

- `missing_view_section_comments` relies on valid headings to create sections.
- `duplicate_view_section_comments` compares normalized valid content.
- `markup_repeating_view_comments` evaluates semantic usefulness after syntax is valid.
- `oversized_view_sections` measures the region owned by each valid heading.

## Open decisions

- Whether view headings reuse the code-phase prefix configuration or have independent settings.
- Whether a blank line is required before headings after the first section.
- Whether one-word headings are syntactically valid even if semantically weak.
- Whether doc comments and block comments are categorically rejected as section headings.
- Whether comments generated inside local macros are ignored or diagnosed at their definition.
