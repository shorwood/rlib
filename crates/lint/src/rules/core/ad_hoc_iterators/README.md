# `rlib::ad_hoc_iterators`

## Summary

Finds unique hand-written inherent methods taking only `&mut self`, returning `Option<T>`, and returning a value derived from persistent cursor-like receiver state while advancing that same state.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds unique hand-written inherent methods taking only `&mut self`, returning `Option<T>`, and
returning a value derived from persistent cursor-like receiver state while advancing that same
state. Explicit cursor/index updates and returned delegation to an inner iterator are recognized
without requiring an observed caller.

## Why this matters

An ad hoc `next_token` protocol cannot participate in `for`, iterator adapters, `collect`,
or generic iterator consumers. It also invents private exhaustion semantics that readers
must rediscover from the body.

## Examples

### Triggers the lint

```rust
struct Tokens { values: Vec<String>, cursor: usize }
impl Tokens {
    fn next_token(&mut self) -> Option<String> {
        let value = self.values.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(value)
    }
}
```

### Use this instead

Queue removal, temporary readiness, receiving, parsing, lending references, contextual
traversal, reusable collections, competing traversals, and existing `Iterator` or
Existing `Iterator` and `IntoIterator` implementations remain valid. The lint never assumes
`FusedIterator`.

```rust
impl Iterator for Tokens {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.values.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(value)
    }
}
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
