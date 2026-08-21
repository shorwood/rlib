# `rlib::derive_more_manual_formatting_impls`

## Summary

Finds standard formatting implementations that only delegate to fields, use one reproducible `write!`, or map enum variants to static text.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard formatting implementations that delegate to a field, call `Formatter::write_str`
with a field, use one reproducible `write!`, or exhaustively map unit variants to static text.

## Why this matters

Hand-written formatting plumbing obscures a declarative display rule and can drift across the
standard formatting trait family.

## Examples

### Triggers the lint

```rust,ignore
impl Display for Dependency {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} -> {}", self.component, self.path.display())
    }
}
```

### Use this instead

Declare the formatting rule on the type, retaining any exact format string in the matching
derive attribute.

```rust,ignore
#[derive(derive_more::Display)]
#[display("{} -> {}", component, path.display())]
struct Dependency { component: String, path: PathBuf }
```

## What it skips

Skips control flow, multiple formatting operations, shadowed macros, and behavioral attributes.
Rustdoc does not change structural recognition; use an explicit lint control for a deliberate
exception. Enum mappings are only reported when `enum-display-provider` selects
`derive_more_display`.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-display-provider` | string | not set | Selects the configured enum display derive. Exhaustive enum mappings are reported only for `derive_more_display`; `strum_display` leaves them to the Strum lint family. |

## Known limitations

No known implementation limitations.

## Related lints

None.
