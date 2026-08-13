# ambiguous_primitive_parameters

## What it does

Finds function and method signatures where parameters with the same primitive
representation carry distinct domain roles. Scalar aliases are resolved semantically, and
owned and borrowed UTF-8 strings form one textual family. Conventional coordinates,
bounds, operands, ranges, dimensions, and generic text operations remain accepted.

## Why is this bad?

Primitive values communicate representation but not meaning. Swapping two identifiers,
limits, or credentials remains valid Rust and may survive review because the call contains
no type-level explanation of either position. Generated code is especially prone to
extending such signatures instead of discovering the domain concepts they represent.

For example, every argument here is interchangeable:

## Example

```rust
fn schedule(user: u64, project: u64, delay: u64) {}
```

## Use instead

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
