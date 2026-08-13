# overloaded_declaration_sections

## What it does

Rejects a valid section when it both exceeds the configured declaration limit and mixes
several independently named concepts. A named type and all of its implementation blocks
count as one declaration. A large but consistently named family remains valid because its
divider still communicates one responsibility. The default scale limit is five declarations.

## Why is this bad?

Declaration count alone does not prove that a section has too many responsibilities. When a
large section also contains unrelated naming families, however, its prefix usually describes
a file-wide topic rather than a followable concept. The remedy is naming-first: identify the
concepts that already exist, give related declarations coherent names, and introduce a second
section only when it represents an independent family.

For example, a broad transport section can hide unrelated request and response concepts:

## Example

```rust
// -----------------------------------------------------------------------------
// Transport: Request and response transport
// -----------------------------------------------------------------------------
struct Request;
struct RequestBuilder;
struct RequestHeaders;
struct Response;
struct ResponseBuilder;
struct TransportError;
```

## Use instead

Make the conceptual families visible in both names and sections:

```rust
// -----------------------------------------------------------------------------
// Request: Request model family
// -----------------------------------------------------------------------------
struct Request;
struct RequestBuilder;
struct RequestHeaders;
struct RequestBody;

// -----------------------------------------------------------------------------
// Response: Response model family
// -----------------------------------------------------------------------------
struct Response;
struct ResponseBuilder;
struct ResponseHeaders;
struct ResponseBody;
```

Configure the scale threshold through `section_dividers.max_declarations_per_section`.
Sections at the maximum are accepted. Larger sections are rejected only when their names
also show more than one conceptual family.
