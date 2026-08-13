# strum_defaulted_payload_enum_construction

## What it does

Finds `EnumIter`, `EnumString`, or `FromRepr` derives that construct data-bearing variants with
default payload values.

## Why is this bad?

Identifiers, quantities, paths, timestamps, and other payload state cannot generally be invented
without domain context. A syntactically valid default can therefore create a semantically invalid
enum value.

## Example

```rust,ignore
#[derive(strum::EnumString)]
enum Limit { Unlimited, Fixed(u32) }
```

## Use instead

Disable the payload variant for that derive, use a unit discriminant enum, or keep an hand-written
constructor that requires the payload.

```rust,ignore
enum Job { Pending(JobId), Complete }
impl Job { fn pending(id: JobId) -> Self { Self::Pending(id) } }
```
