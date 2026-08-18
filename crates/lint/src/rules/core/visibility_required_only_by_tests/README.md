# `rlib::visibility_required_only_by_tests`

## Summary

Compares the visibility required by production references with the visibility required when standard in-source `#[cfg(test)] mod test` or `#[cfg(test)] mod tests` modules are included.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Compares the visibility required by production references with the visibility required when
standard in-source `#[cfg(test)] mod test` or `#[cfg(test)] mod tests` modules are included.
The explicit configuration gate is required, so a production module merely named `tests` is
not reclassified. The lint diagnoses declarations whose hand-written standard visibility is
justified only by those tests. The rule covers module items, types, functions, constants,
statics, struct and union fields, and inherent associated items. Separate end-to-end test
crates are intentionally outside this crate-local analysis.

## Why this matters

Tests should verify a production boundary, not silently create it. A sibling test module
that reaches through `pub(crate)` can turn internal implementation details into a crate-wide
names, couple tests to file layout, and prevent the defining module from owning its
invariants. This warning makes that test-induced design decision visible.

## Examples

### Triggers the lint

```rust
mod parser {
    pub(crate) fn normalize(input: &str) -> String {
        input.trim().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::parser;

    #[test]
    fn normalizes_input() {
        assert_eq!(parser::normalize(" x "), "x");
    }
}
```

### Use this instead

Prefer colocating a private unit test inside `parser`, or test normalization through the
public behavior that owns it. The lint does not offer an automatic edit because moving tests
or choosing a behavioral seam is an architectural decision.

```rust
mod parser {
    fn normalize(input: &str) -> String {
        input.trim().to_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::normalize;

        #[test]
        fn normalizes_input() {
            assert_eq!(normalize(" x "), "x");
        }
    }
}
```

## What it skips

The explicit configuration gate is required, so a production module merely named `tests` is not reclassified. Separate end-to-end test crates are intentionally outside this crate-local analysis.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::unnecessarily_broad_visibility`](../unnecessarily_broad_visibility/README.md) — Finds visibility wider than all current uses require.
- [`rlib::noncanonical_restricted_visibility`](../noncanonical_restricted_visibility/README.md) — Limits the forms used to express visibility.
