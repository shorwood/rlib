# ad_hoc_conversions

## What it does

Finds unique, effect-free functions and inherent methods that consume exactly one concrete
source value and construct one distinct local target. A direct target return is treated as
an infallible `From` opportunity; an exact `Result<Target, Error>` return is treated as a
fallible `TryFrom` opportunity.

The analysis follows the source through destructuring, local bindings, assignments,
branches, matches, closures, and helper calls. It accepts concrete nominal, primitive,
tuple, array, slice, and reference sources while rejecting unresolved generic, opaque,
dynamic, and raw-pointer contracts. Lifetimes do not split otherwise identical families.

## Why is this bad?

Ad hoc conversion functions hide standard capabilities from readers, generic code, IDEs,
and trait-driven APIs. They also invite several names for the same semantic pair, making it
unclear which conversion is canonical.

## Example

```rust
struct Record(String);
struct Account(String);

fn account_from_record(record: Record) -> Account {
    Account(record.0)
}
```

## Use instead


Prefer declaring conversion ownership on the target:

```rust
struct Record(String);
struct Account(String);

impl From<Record> for Account {
    fn from(record: Record) -> Self {
        Self(record.0)
    }
}
```

Families with multiple candidates, existing `From` or `TryFrom` implementations, policy or
effect vocabulary, ambient static state, and known standard I/O effects are left alone.
Exact owned `&str -> Result<T, E>` parsers remain the responsibility of
`ad_hoc_string_parsers`. No automatic fix is offered because moving an API into a trait can
change visibility, coherence, error contracts, and call syntax.
