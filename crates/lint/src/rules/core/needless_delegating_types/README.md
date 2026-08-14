# needless_delegating_types

## What it does

Finds hand-written concrete single-field structs whose complete inherent API contains at least
two behavior-bearing methods and only forwards them to the stored value. Forwarding must
preserve the receiver, arguments and their order, return value, error propagation, and async
behavior. An identity constructor or direct field accessor is neutral, but any substantive
constructor or method preserves the wrapper.
Receiver preservation distinguishes owned, shared-borrowed, and mutably borrowed calls, including
when a `Copy` inner value would otherwise make a consuming call compile through a wrapper borrow.

The lint excludes generic wrappers and wrappers with trait implementations, since those can
own a real type-level or behavioral contract. Semantic attributes such as representation or
serialization policy also preserve the type; documentation and lint-control attributes do
not establish such a contract by themselves. Generated declarations and unrestricted public
APIs in publishable libraries are ignored. Binaries and packages marked `publish = false`
are treated as closed, and behavior compiled for active tests is included in the decision.

## Why is this bad?

A wrapper without an invariant, representation boundary, or behavioral policy adds another
name and navigation layer without owning a distinct contract. Using the stored type directly
keeps ownership visible until the wrapper has real behavior to enforce.

## Example

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

## Use instead

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
signatures, imports, and potentially external data or ABI contracts.
