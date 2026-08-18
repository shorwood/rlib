# `rlib::bon_manual_builder_implementations`

## Summary

Finds dedicated `*Builder` structs with a start method, distinct field-named consuming setters covering their state, and an infallible terminal method producing a named type that Bon can generate.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds dedicated `*Builder` structs with a start method, distinct field-named consuming setters
covering their state, and an infallible terminal method producing a named type that Bon can
generate. Unrelated consuming fluent methods and scalar-computing terminal methods do not count.

## Why this matters

Hand-maintained field-based builders repeat setter plumbing and construction steps while lacking
Bon's compile-time required-member and duplicate-setter guarantees.

## Examples

### Triggers the lint

```rust,ignore
struct RequestBuilder { host: String, port: u16 }
// `new`, consuming `host`/`port` setters, and `build` are implemented manually.
```

### Use this instead

Use `#[derive(bon::Builder)]` for field-based construction, or `#[builder]` on a constructor that owns
validation or normalization.

```rust,ignore
#[derive(bon::Builder)]
struct Request {
    host: String,
    port: u16,
}
```

## What it skips

Unrelated consuming fluent methods and scalar-computing terminal methods do not count.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
