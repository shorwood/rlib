# strum_declaration_order_domain_contracts

## What it does

Finds `EnumIter` or `VariantArray` declaration order used directly by workflow, migration,
priority, protocol, menu, or presentation APIs.

## Why is this bad?

Reordering enum declarations then changes execution or presentation behavior without changing an
explicit ordering policy.

## Example

```rust,ignore
fn execute_migration() {
    for phase in MigrationPhase::iter() { execute(phase); }
}
```

## Use instead

Expose a named ordering constant or method, or sort by an explicit key before consuming variants.

```rust,ignore
const MIGRATION_ORDER: [MigrationPhase; 2] = [MigrationPhase::Prepare, MigrationPhase::Apply];
```
