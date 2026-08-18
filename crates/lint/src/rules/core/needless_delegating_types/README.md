# `rlib::needless_delegating_types`

## Summary

Finds hand-written concrete single-field structs whose complete inherent API contains at least two behavior-bearing methods and only forwards them to the stored value.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written concrete single-field structs whose complete inherent API contains at least
two behavior-bearing methods and only forwards them to the stored value. Forwarding must
preserve the receiver, arguments and their order, return value, error propagation, and async
behavior. An identity constructor or direct field accessor is neutral, but any substantive
constructor or method preserves the wrapper.
Receiver preservation distinguishes owned, shared-borrowed, and mutably borrowed calls, including
when a `Copy` inner value would otherwise make a consuming call compile through a wrapper borrow.

The lint excludes generic wrappers and wrappers with trait implementations, since those can
have a distinct purpose. Attributes that affect representation or serialization also preserve
the type; documentation and lint-control attributes do not justify a separate type by themselves.
Generated declarations and unrestricted public
APIs in publishable libraries are ignored. Binaries and packages marked `publish = false`
are treated as closed, and behavior compiled for active tests is included in the decision.

## Why this matters

A wrapper without its own validation, representation, or behavior adds another
name and navigation layer without owning a distinct purpose. Using the stored type directly
keeps ownership visible until the wrapper has real behavior to enforce.

## Examples

### Triggers the lint

```rust
struct Connection;

impl Connection {
    fn read(&self) -> usize { 0 }
    fn write(&self, bytes: usize) -> usize { bytes }
}

struct Client(Connection);

impl Client {
    fn read(&self) -> usize {
        self.0.read()
    }

    fn write(&self, bytes: usize) -> usize {
        self.0.write(bytes)
    }
}
```

### Use this instead

If `Client` owns no validation, lifecycle, representation, or domain policy, use the stored
type directly:

```rust
struct Connection;

impl Connection {
    fn read(&self) -> usize { 0 }
    fn write(&self, bytes: usize) -> usize { bytes }
}

fn transfer(connection: &Connection) {
    let bytes = connection.read();
    connection.write(bytes);
}
```

Exact forwarding is intentionally a narrow signal: wrappers that transform values, enforce
invariants, implement traits, or contain any substantive API are not diagnosed. No automatic
rewrite is offered because removing a named type changes construction, field access, type
signatures, imports, and potentially external data formats or ABI guarantees.

## What it skips

It ignores generic wrappers, wrappers with trait implementations, types with representation or serialization attributes, generated declarations, and unrestricted public APIs in publishable libraries.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::needless_function_wrappers`](../needless_function_wrappers/README.md) — Covers functions that only forward a call.
- [`rlib::single_implementation_traits`](../single_implementation_traits/README.md) — Covers traits that add no current choice of implementation.
