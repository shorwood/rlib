# bool_fields_without_predicate_prefix

## What it does

Checks that named boolean fields in structs use `is_<predicate>` or `has_<predicate>`. The prefix
must be followed by a nonempty, ordinarily formed predicate phrase.

## Why is this bad?

A predicate prefix makes the meaning of a boolean field clear at call sites. Without one,
the field can read like a command, an event, or an arbitrary value instead of a yes-or-no
property.

For example, these field names do not communicate that they are predicates:

## Example

```rust
struct Window {
    active: bool,
    children: bool,
}
```

## Use instead

Prefixing them makes their role explicit wherever the fields are read:

```rust
struct Window {
    is_active: bool,
    has_children: bool,
}
```
