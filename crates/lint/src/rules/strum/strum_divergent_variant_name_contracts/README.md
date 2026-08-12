# strum_divergent_variant_name_contracts

## What it does

Finds `VariantNames` contracts that expose one preferred spelling while `EnumString` accepts a
different alias vocabulary for the same variants.

## Why is this bad?

Callers commonly use `VARIANTS` as the advertised set of accepted values. Hidden aliases make that
vocabulary incomplete and can produce inconsistent validation or help text.

## Example

```rust,ignore
#[derive(strum::VariantNames, strum::EnumString)]
enum Color { #[strum(serialize = "gray", serialize = "grey")] Gray }
```

## Use instead

Expose a deliberate canonical-name API and document aliases separately.

```rust,ignore
const CANONICAL_NAMES: &[&str] = &["pending", "complete"];
```
