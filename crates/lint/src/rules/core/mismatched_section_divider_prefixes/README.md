# mismatched_section_divider_prefixes

## What it does

Requires a divider prefix to equal the longest PascalCase word prefix shared by every
participating type, free function, constant, and static in its section. Rust identifier
conventions are normalized before comparison, so `request_parser` belongs to `Request`.
A divider may instead match the containing module's name: the module then supplies the
family namespace, allowing idiomatic APIs such as `identifier_case::words` without forcing
the redundant name `identifier_case::identifier_words`.

## Why is this bad?

A mismatched or vague prefix conceals inconsistent names. The preferred remedy is to rename
related outliers into a followable family, not to create a section for every declaration.

For example, Response does not belong to the declared Request family:

## Example

```rust
// -----------------------------------------------------------------------------
// Request
// -----------------------------------------------------------------------------
struct Request;
struct Response;
```

## Use instead


Rename a related declaration so the family is visible:

```rust
// -----------------------------------------------------------------------------
// Request
// -----------------------------------------------------------------------------
struct Request;
struct RequestResponse;
```

A module-owned namespace is also valid even when its declarations have different names:

```rust
mod identifier_case {
    // -----------------------------------------------------------------------------
    // IdentifierCase
    // -----------------------------------------------------------------------------
    pub fn words(value: &str) -> Vec<String> {
        todo!()
    }

    pub fn is_pascal(value: &str) -> bool {
        todo!()
    }
}
```
