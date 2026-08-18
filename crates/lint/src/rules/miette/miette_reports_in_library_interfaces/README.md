# `rlib::miette_reports_in_library_interfaces`

## Summary

Finds exported functions and methods in library crate types that return `miette::Report` directly or through `miette::Result`, including behind standard owning pointers.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds exported functions and methods in library crate types that return `miette::Report` directly
or through `miette::Result`, including behind standard owning pointers.

## Why this matters

Reports belong at application rendering boundaries. Returning one from a library erases the
concrete failure names callers need for classification and recovery.

## Examples

### Triggers the lint

```rust,ignore
pub fn load() -> miette::Result<()> { todo!() }
```

### Use this instead

```rust,ignore
pub fn load() -> Result<(), LoadError> { todo!() }
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
