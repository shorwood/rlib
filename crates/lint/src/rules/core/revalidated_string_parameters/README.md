# `rlib::revalidated_string_parameters`

## Summary

Finds domain-shaped textual parameters whose invariants are established near the start of at least two consumers in the same module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds domain-shaped textual parameters whose invariants are established near the start of
at least two consumers in the same module. Validation and normalization helper calls and
parameter-dependent early rejection guards count as evidence. The constructor, parser, or
validator that establishes the boundary is not itself a consumer. Parameter-dependent fast paths
that directly return standard `Ok` or `Some` success are not rejection evidence.

## Why this matters

Revalidating raw text at every use means the type permits invalid states throughout the
program. Every new consumer must remember the same checks, normalization can drift, and an
agent can accidentally bypass the invariant by adding one apparently harmless `&str`
parameter.

## Examples

### Triggers the lint

```rust
fn send_invitation(email: &str) -> Result<()> {
    validate_email(email)?;
    deliver(email)
}

fn subscribe(email: &str) -> Result<()> {
    validate_email(email)?;
    store(email)
}
```

### Use this instead

Establish validity once and make invalid construction impossible:

```rust
struct EmailAddress(String);

fn send_invitation(email: &EmailAddress) -> Result<()> {
    deliver(email)
}
```

## What it skips

The constructor, parser, or validator that establishes the boundary is not itself a consumer. Parameter-dependent fast paths that directly return standard `Ok` or `Some` success are not rejection evidence.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
