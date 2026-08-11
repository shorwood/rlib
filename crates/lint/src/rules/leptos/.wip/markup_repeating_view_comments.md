# Markup-repeating view comments

## Proposition

Add a `markup_repeating_view_comments` lint for comments in Leptos views that merely restate the
immediately following tag, component, attribute, event, or visible literal.

```rust
// Bad: the markup already communicates every comment.
// Header
<header>...</header>

// Navigation
<nav>...</nav>

// Submit button
<button type="submit">"Submit"</button>
```

```rust
// Better: comments supply responsibility or policy absent from the markup.
// Primary account navigation
<nav>...</nav>

// Submission remains unavailable while validation is pending
<button type="submit" disabled=move || !can_submit.get()>
    "Submit"
</button>
```

## Rationale

Generated code often comments syntax instead of intent. These comments add vertical noise, create
maintenance obligations, and give a false impression of documentation without helping readers
understand ownership or behavior. View-section requirements must not reward this failure mode.

The purpose of a view comment is to name a multi-node responsibility, explain non-obvious policy, or
make an invisible interaction constraint discoverable.

## Conservative detection

Compare normalized comment words with structural evidence from the following node:

- native tag name;
- component name and its identifier words;
- `id`, dominant class, or role value;
- event-handler name;
- literal child text;
- a single obvious prop name.

High-confidence examples include `Header` before `header`, `User profile` before `UserProfile`, and
`Click handler` before an `on:click` attribute. Partial word overlap alone is not enough.

## Section comments versus item comments

```rust
// User profile
<UserProfile/>
```

This is likely redundant when the comment owns only that component. The same heading may be useful
when it names a region containing several profile-related nodes whose collective responsibility is
not otherwise explicit.

The analysis therefore needs section ownership from `missing_view_section_comments`, not just
adjacent-token comparison.

## Non-redundant explanations

Comments that explain browser behavior, accessibility policy, security boundaries, hydration
constraints, or product decisions may remain valid even when they mention the affected element.
The rule should stay conservative and avoid NLP claims beyond strong structural repetition.

## Diagnostic direction

> `Submit button` repeats the following button and its visible label. Remove the comment, or replace
> it with the non-obvious responsibility or policy this node participates in.

The lint must never generate a more verbose replacement automatically.

## Open decisions

- The amount of normalized token overlap required for a deterministic finding.
- Whether one-component section headings are always considered redundant.
- Whether comments naming accessibility landmarks are useful despite repeating semantic tags.
- How literal localization keys and translated content affect visible-text comparison.
- Whether generic comments such as `Content` belong here or a separate weak-heading rule.
