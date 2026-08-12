# incoherent_type_family_names

## What it does

Finds type names that repeat the module or lint-pass name instead of expressing the smaller
concept that connects those declarations. It analyzes valid sections and unsectioned module
declarations, combining normalized Rust identifier words with compiler-resolved
dependencies and source proximity.

## Why is this bad?

Names should determine the useful sections, not be lengthened merely to satisfy a divider.
Repeating broad organizational context hides roles, makes related helpers harder to scan,
and encourages agents to solve naming problems by creating more singleton sections.

For example, these helpers repeat the enclosing lint name:

## Example

```rust
// -----------------------------------------------------------------------------
// MethodLikeFreeFunctions
// -----------------------------------------------------------------------------

struct MethodLikeFreeFunctions;
struct MethodLikeFreeFunctionsSourceEdits;
struct MethodLikeFreeFunctionsMigrationBuilder {
    edits: MethodLikeFreeFunctionsSourceEdits,
}
```

## Use instead


Naming the smaller concept first produces a followable family:

```rust
// -----------------------------------------------------------------------------
// Migration
// -----------------------------------------------------------------------------

struct MigrationEdits;
struct MigrationBuilder {
    edits: MigrationEdits,
}
```
