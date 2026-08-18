# framework_resolution_required

## What it does

Reports a derivable contract when several enabled libraries could provide it and the project has
not chosen which library owns that contract.

## Why is this bad?

The libraries may produce similar code today while exposing different traits, helper methods, and
upgrade paths. Choosing whichever dependency happens to be present makes public behavior change for
reasons unrelated to the source being linted.

## Example

```rust,ignore
impl Stage {
    const ALL: [Self; 2] = [Self::Planned, Self::Complete];
}
```

## Use instead

Select the intended provider explicitly in the workspace-root `dylint.toml`:

```rust,ignore
[rlib-lint]
enum-variant-collection-provider = "strum_variant_array"
enum-variant-predicate-provider = "strum_enum_is"
enum-display-provider = "strum_display"
enum-string-parsing-provider = "strum_enum_string"
error-implementation-provider = "thiserror_error"
error-variant-conversion-provider = "thiserror_from"
```

Only configure choices that apply to the enabled libraries. The lint does not pick a provider or
silently prefer one based on dependency order.

## Configuration

The six flat `*-provider` keys select an implementation when multiple enabled frameworks can satisfy the same contract.
