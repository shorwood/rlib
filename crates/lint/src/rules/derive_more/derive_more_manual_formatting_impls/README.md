# `rlib::derive_more_manual_formatting_impls`

## Summary

Finds standard formatting implementations that only delegate to one field or render that field through one `write!` invocation.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard formatting implementations that only delegate to one field or render that field
through one `write!` invocation.

## Why this matters

Hand-written formatting plumbing obscures a declarative display rule and can drift across the
standard formatting trait family.

## Examples

### Triggers the lint

```rust,ignore
impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
```

### Use this instead

Declare the formatting rule on the type, retaining any exact format string in the matching
derive attribute.

```rust,ignore
#[derive(derive_more::Display)]
struct UserId(u64);
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
