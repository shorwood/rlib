# overloaded_declaration_sections

## What it does

Rejects a valid section divider when it governs more distinct declarations than the
configured maximum. A nominal type and all of its implementation blocks count as one
declaration, while separate types, free functions, constants, and statics count
independently. The default maximum is five declarations per section.

## Why is this bad?

An oversized section usually means its prefix describes a file-wide topic rather than one
followable concept. That broad bucket conceals smaller naming families and encourages new
declarations to accumulate without an explicit owner. The remedy is naming-first: identify
the concepts that already exist, give related declarations coherent names, and introduce a
second section only when it represents an independent family.

For example, a broad transport section can hide unrelated request and response concepts:

## Example

```rust
// -----------------------------------------------------------------------------
// Transport
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
// Request
// -----------------------------------------------------------------------------
struct Request;
struct RequestBuilder;
struct RequestHeaders;
struct RequestBody;

// -----------------------------------------------------------------------------
// Response
// -----------------------------------------------------------------------------
struct Response;
struct ResponseBuilder;
struct ResponseHeaders;
struct ResponseBody;
```

Configure the threshold through
`section_dividers.max_declarations_per_section`. Sections at the maximum are accepted;
only sections above it are rejected.
