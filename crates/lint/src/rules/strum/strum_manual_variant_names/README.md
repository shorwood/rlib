# strum_manual_variant_names

## What it does

Finds static string tables that repeat every enum variant's canonical Strum name in declaration
order.

## Why is this bad?

The table must be updated whenever variants or naming attributes change. `VariantNames` derives the
same ordered vocabulary from the enum.

## Example

```rust,ignore
enum Stage { Planned, Active }
const STAGE_NAMES: &[&str] = &["Planned", "Active"];
```

## Use instead

Derive `strum::VariantNames` and use `Stage::VARIANTS`. Review public table types before migrating.

```rust,ignore
#[derive(strum::VariantNames)]
enum Stage { Planned, Complete }
```
