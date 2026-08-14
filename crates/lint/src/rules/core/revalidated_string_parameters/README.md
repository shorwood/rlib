# revalidated_string_parameters

## What it does

Finds domain-shaped textual parameters whose invariants are established near the start of
at least two consumers in the same module. Validation and normalization helper calls and
parameter-dependent early rejection guards count as evidence. The constructor, parser, or
validator that establishes the boundary is not itself a consumer. Parameter-dependent fast paths
that directly return standard `Ok` or `Some` success are not rejection evidence.

## Why is this bad?

Revalidating raw text at every use means the type permits invalid states throughout the
program. Every new consumer must remember the same checks, normalization can drift, and an
agent can accidentally bypass the invariant by adding one apparently harmless `&str`
parameter.

## Example

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

## Use instead

Establish validity once and make invalid construction impossible:

```rust
struct EmailAddress(String);

fn send_invitation(email: &EmailAddress) -> Result<()> {
    deliver(email)
}
```
