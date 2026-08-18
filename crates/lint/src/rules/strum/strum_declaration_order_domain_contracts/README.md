# `rlib::strum_declaration_order_domain_contracts`

## Summary

Finds `EnumIter` or `VariantArray` declaration order used directly by workflow, migration, priority, protocol, menu, or presentation APIs.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `EnumIter` or `VariantArray` declaration order used directly by workflow, migration,
priority, protocol, menu, or presentation APIs.

## Why this matters

Reordering enum declarations then changes execution or presentation behavior without changing an
explicit ordering policy.

## Examples

### Triggers the lint

```rust,ignore
fn execute_migration() {
    for phase in MigrationPhase::iter() { execute(phase); }
}
```

### Use this instead

Expose a named ordering constant or method, or sort by an explicit key before consuming variants.

```rust,ignore
const MIGRATION_ORDER: [MigrationPhase; 2] = [MigrationPhase::Prepare, MigrationPhase::Apply];
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
