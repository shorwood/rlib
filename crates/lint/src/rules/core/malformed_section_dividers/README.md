# malformed_section_dividers

## What it does

Validates section-divider content and rendered width against the configured template.
Content names a PascalCase abstraction and its sentence-style responsibility, separated by a
colon. The responsibility starts with a capital letter while preserving authored proper nouns,
acronyms, and hyphenated technical terms. A bare abstraction name is incomplete because it does
not explain why the declarations belong together.

## Why is this bad?

Inconsistent syntax weakens dividers as navigation landmarks and makes generated edits
unpredictable. A name without a responsibility merely repeats the declaration below it, while
empty or overlong sections hide rather than clarify source structure.

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
