# ad_hoc_ordering

## What it does

Finds neutral free functions and inherent methods over two shared references to one local
type. A proven `Ordering` result is treated as an `Ord` opportunity; a proven
`Option<Ordering>` result is treated as a `PartialOrd` opportunity.

## Why is this bad?

Named canonical comparators hide ordering from generic code and ordered collections while
allowing equality, sorting, and deduplication to evolve independently. Several unqualified
comparators make the ambiguity worse rather than establishing useful domain vocabulary.

## Example

```rust
use std::cmp::Ordering;
struct Slug(String);
fn compare_slugs(left: &Slug, right: &Slug) -> Ordering {
    left.0.cmp(&right.0)
}
```

## Use instead

Contextual orderings such as display order, priority, or business-key order remain named.
Float-backed types are ignored unless the body explicitly establishes total ordering.

```rust
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct Slug(String);
```
