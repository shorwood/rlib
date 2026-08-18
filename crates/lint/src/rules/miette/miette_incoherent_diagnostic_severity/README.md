# `rlib::miette_incoherent_diagnostic_severity`

## Summary

Finds statically advisory Miette diagnostic types used as the error channel of `Result`-returning functions and methods, including through standard owning pointer wrappers.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds statically advisory Miette diagnostic types used as the error channel of `Result`-returning
functions and methods, including through standard owning pointer wrappers. A type-level enum
severity is included because it applies to every variant.

## Why this matters

Declaring a warning or advice while propagating it as a failure gives reporters and control flow
contradictory classifications.

## Examples

### Triggers the lint

```rust,ignore
#[diagnostic(severity(Warning))]
struct DeprecatedInput;

fn validate() -> Result<(), DeprecatedInput> { todo!() }
```

### Use this instead

Use error severity for failures, or collect the diagnostic as advisory output while continuing.

```rust,ignore
fn validate() -> Result<Vec<DeprecatedInput>, ValidationError> { todo!() }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
