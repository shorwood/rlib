# derive_more_manual_formatting_impls

## What it does

Finds standard formatting implementations that only delegate to one field or render that field
through one `write!` invocation.

## Why is this bad?

Authored formatting plumbing obscures a declarative presentation contract and can drift across the
standard formatting trait family.

## Example

```rust,ignore
impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
```

## Use instead

Declare the formatting contract on the type, retaining any exact format string in the matching
derive attribute.

```rust,ignore
#[derive(derive_more::Display)]
struct UserId(u64);
```
