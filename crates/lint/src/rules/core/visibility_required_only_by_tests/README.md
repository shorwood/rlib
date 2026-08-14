# visibility_required_only_by_tests

## What it does

Compares the visibility required by production references with the visibility required when
canonical in-source `#[cfg(test)] mod test` or `#[cfg(test)] mod tests` modules are included.
The explicit configuration gate is required, so a production module merely named `tests` is
not reclassified. The lint diagnoses declarations whose hand-written canonical visibility is
justified only by those tests. The rule covers module items, types, functions, constants,
statics, struct and union fields, and inherent associated items. Separate end-to-end test
crates are intentionally outside this crate-local analysis.

## Why is this bad?

Tests should verify a production boundary, not silently create it. A sibling test module
that reaches through `pub(crate)` can turn internal implementation details into a crate-wide
vocabulary, couple tests to file layout, and prevent the defining module from owning its
invariants. This warning makes that test-induced design decision visible.

## Example

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

## Use instead

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
