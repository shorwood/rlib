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
[rlib-lint.derive_resolution]
enum_variant_collection = "strum_variant_array"
enum_variant_predicates = "strum_enum_is"
enum_display = "strum_display"
enum_string_parsing = "strum_enum_string"
error_implementation = "thiserror_error"
error_variant_conversion = "thiserror_from"
```

Only configure choices that apply to the enabled libraries. The lint does not pick a provider or
silently prefer one based on dependency order.
