# `rlib::bon_implicit_optional_builder_members`

## Summary

Finds policy-bearing `Option<T>` arguments on Bon-generated free-function and inherent-method builders that Bon makes omittable without an explicit policy.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds policy-bearing `Option<T>` arguments on Bon-generated free-function and inherent-method
builders that Bon makes omittable without an explicit policy.

## Why this matters

Omitting a timeout, authorization decision, limit, destination, or callback can carry domain meaning.
An implicitly optional setter makes an accidental omission indistinguishable from a deliberate choice.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
fn request(timeout: Option<Duration>, url: String) {}
```

### Use this instead

Require callers to provide the optional value explicitly, document intentional omission, or model the
choice with a behavioral enum:

```rust,ignore
#[bon::builder]
fn request(#[builder(required)] timeout: Option<Duration>, url: String) {}
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
