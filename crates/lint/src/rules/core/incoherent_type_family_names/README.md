# `rlib::incoherent_type_family_names`

## Summary

Finds type names that repeat the module or lint-pass name instead of expressing the smaller concept that connects those declarations.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds type names that repeat the module or lint-pass name instead of expressing the smaller
concept that connects those declarations. It analyzes valid sections and unsectioned module
declarations, combining normalized Rust identifier words with compiler-resolved
dependencies and source proximity.
Generated declarations are excluded as naming candidates but still reserve their identifiers, so
rename guidance does not propose an occupied type name.

## Why this matters

Names should determine the useful sections, not be lengthened merely to satisfy a divider.
Repeating broad organizational context hides roles, makes related helpers harder to scan,
and encourages agents to solve naming problems by creating more singleton sections.

For example, these helpers repeat the enclosing lint name:

## Examples

### Triggers the lint

```rust
// -----------------------------------------------------------------------------
// MethodLikeFreeFunctions: Migration support
// -----------------------------------------------------------------------------

struct MethodLikeFreeFunctions;
struct MethodLikeFreeFunctionsSourceEdits;
struct MethodLikeFreeFunctionsMigrationBuilder {
    edits: MethodLikeFreeFunctionsSourceEdits,
}
```

### Use this instead

Naming the smaller concept first produces a followable family:

```rust
// -----------------------------------------------------------------------------
// Migration: Source migration support
// -----------------------------------------------------------------------------

struct MigrationEdits;
struct MigrationBuilder {
    edits: MigrationEdits,
}
```

## What it skips

Generated declarations are excluded as naming candidates but still reserve their identifiers, so rename guidance does not propose an occupied type name.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
