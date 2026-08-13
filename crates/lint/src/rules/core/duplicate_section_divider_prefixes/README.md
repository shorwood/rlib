# duplicate_section_divider_prefixes

## What it does

Rejects repeated section-divider prefixes within the same module. Nested modules have
independent prefix namespaces.

## Why is this bad?

Repeating a prefix fragments one naming family and makes it unclear which section owns new
declarations. Closely related declarations should remain together.

For example, this module splits the Request family:

## Example

```rust
// -----------------------------------------------------------------------------
// Request: Request model
// -----------------------------------------------------------------------------
struct Request;

// -----------------------------------------------------------------------------
// Request: Request construction
// -----------------------------------------------------------------------------
struct RequestBuilder;
```

## Use instead

Keep the family under one divider:

```rust
// -----------------------------------------------------------------------------
// Request: Request model and construction
// -----------------------------------------------------------------------------
struct Request;
struct RequestBuilder;
```
