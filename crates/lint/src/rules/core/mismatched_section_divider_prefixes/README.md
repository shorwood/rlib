# mismatched_section_divider_prefixes

## What it does

Requires a divider prefix to equal the longest PascalCase word prefix shared by the
types in its section. When a section has no types, its free functions, constants, and
statics establish the family instead. Rust identifier conventions are normalized before
comparison, so `request_parser` belongs to `Request`.
A divider may instead match the containing module's name: the module then supplies the
family namespace, allowing idiomatic APIs such as `identifier_case::words` without forcing
the redundant name `identifier_case::identifier_words`.

`Violation` is a deliberate role-based exception. A rule's diagnostic and its local
evidence types may share that section without artificial `Violation…` prefixes. A section
may also use the name of its principal abstraction when every declaration shares a parent
module namespace, such as `SerdeAttributes` beside other `Serde…` contract types. The
library name alone does not prove that all of those types have one responsibility.

## Why is this bad?

A mismatched or vague prefix conceals inconsistent names. The preferred remedy is to rename
related outliers into a followable family, not to create a section for every declaration.

For example, Response does not belong to the declared Request family:

## Example

```rust
// -----------------------------------------------------------------------------
// Request: Request and response models
// -----------------------------------------------------------------------------
struct Request;
struct Response;
```

## Use instead

Rename a related declaration so the family is visible:

```rust
// -----------------------------------------------------------------------------
// Request: Request model family
// -----------------------------------------------------------------------------
struct Request;
struct RequestResponse;
```

A module-owned namespace is also valid even when its declarations have different names:

```rust
mod identifier_case {
    // -----------------------------------------------------------------------------
    // IdentifierCase: Identifier spelling operations
    // -----------------------------------------------------------------------------
    pub fn words(value: &str) -> Vec<String> {
        todo!()
    }

    pub fn is_pascal(value: &str) -> bool {
        todo!()
    }
}
```

A principal abstraction may own supporting types under a shared library namespace:

```rust
mod serde {
    mod contracts {
        // -----------------------------------------------------------------------------
        // SerdeAttributes: Authored wire policy recovery
        // -----------------------------------------------------------------------------
        struct SerdeDirection;
        struct SerdeAttributes;
        struct SerdeFlag;
    }
}
```
