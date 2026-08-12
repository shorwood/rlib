# framework_resolution_required

## What it does

Reports a framework remediation that has multiple eligible providers but no explicit provider
selection in the `rlib-lint` Dylint configuration.

## Why is this bad?

Choosing a derive provider changes generated traits, methods, and compatibility guarantees. A lint
must not turn dependency presence or source order into an implicit project policy.

## Example

```rust,ignore
impl Stage {
    const ALL: [Self; 2] = [Self::Planned, Self::Complete];
}
```

## Use instead

Select the intended provider explicitly in the workspace-root `dylint.toml`:

```rust,ignore
// dylint.toml: [rlib-lint.derive_resolution]
// enum_variant_collection = "strum_variant_array"
// enum_variant_predicates = "strum_enum_is"
// enum_display = "strum_display"
// enum_string_parsing = "strum_enum_string"
```
