# `rlib::ambiguous_primitive_parameters`

## Summary

Finds function and method signatures where parameters with the same primitive representation carry distinct domain roles.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds function and method signatures where parameters with the same primitive
representation carry distinct domain roles. Scalar aliases are resolved semantically, and
owned and borrowed UTF-8 strings form one textual family. Conventional coordinates,
bounds, operands, ranges, dimensions, and generic text operations remain accepted.
Non-text primitives are grouped only when their ownership and reference mutability also match,
because owned, shared-borrowed, and mutably borrowed arguments are not freely swappable.

## Why this matters

Primitive values communicate representation but not meaning. Swapping two identifiers,
limits, or credentials remains valid Rust and may survive review because the call contains
no type-level explanation of either position. Generated code is especially prone to
extending such signatures instead of discovering the domain concepts they represent.

For example, every argument here is interchangeable:

## Examples

### Triggers the lint

```rust
fn schedule(user: u64, project: u64, delay: u64) {}
```

### Use this instead

Give reusable identities their own types and group operation-specific values in a named
request:

```rust
struct UserId(u64);
struct ProjectId(u64);
struct ScheduleOptions {
    delay: Duration,
}

fn schedule(user: UserId, project: ProjectId, options: ScheduleOptions) {}
```

## What it skips

Non-text primitives are grouped only when their ownership and reference mutability also match, because owned, shared-borrowed, and mutably borrowed arguments are not freely swappable.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
