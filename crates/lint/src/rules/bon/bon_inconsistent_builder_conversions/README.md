# `rlib::bon_inconsistent_builder_conversions`

## Summary

Finds same-typed members of one Bon free-function, associated-function, or derived-struct builder that arbitrarily mix `#[builder(into)]` with strict input types.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds same-typed members of one Bon free-function, associated-function, or derived-struct
builder that arbitrarily mix `#[builder(into)]` with strict input types. Members with an
explicit `with` converter are incomparable and excluded.

## Why this matters

Callers must remember which otherwise equivalent setters accept owned conversions. This makes a
generated named API less predictable than the positional function it replaces.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
fn request(#[builder(into)] url: String, header: String) {}
```

### Use this instead

Apply one deliberate conversion policy to the same representation, or use a custom converter where
the members genuinely have different validation semantics:

```rust,ignore
#[bon::builder]
fn request(#[builder(into)] url: String, #[builder(into)] header: String) {}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
