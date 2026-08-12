# ad_hoc_iterators

## What it does

Finds unique authored inherent methods taking only `&mut self`, returning `Option<T>`, and
advancing persistent cursor-like receiver state. Explicit cursor/index updates and direct
delegation to an inner iterator are recognized without requiring an observed caller.

## Why is this bad?

An ad hoc `next_token` protocol cannot participate in `for`, iterator adapters, `collect`,
or generic iterator consumers. It also invents private exhaustion semantics that readers
must rediscover from the body.

## Example

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

## Use instead

Queue removal, temporary readiness, receiving, parsing, lending references, contextual
traversal, reusable collections, competing traversals, and existing `Iterator` or
`IntoIterator` contracts remain valid. The lint never assumes `FusedIterator`.

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
