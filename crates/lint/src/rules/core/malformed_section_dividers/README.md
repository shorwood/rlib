# malformed_section_dividers

## What it does

Validates section-divider content and rendered width against the configured template.
Content uses a PascalCase prefix with an optional sentence-style description after a colon.
The description starts with a capital letter while preserving authored proper nouns,
acronyms, and hyphenated technical terms.

## Why is this bad?

Inconsistent syntax weakens dividers as navigation landmarks and makes generated edits
unpredictable. Empty or overlong sections also hide rather than clarify source structure.

For example, this divider has non-canonical spacing and description casing:

## Example

```rust
// -----------------------------------------------------------------------------
// Request : request handling
// -----------------------------------------------------------------------------
struct Request;
```

## Use instead

Render the same content canonically:

```rust
// -----------------------------------------------------------------------------
// Request: Request handling
// -----------------------------------------------------------------------------
struct Request;
```
