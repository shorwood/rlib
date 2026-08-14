# method_like_free_functions

## What it does

Checks for free functions whose first parameter can be the receiver of an inherent method
on a struct defined in the same module.
Automatic migrations move generic parameters only when the resolved receiver arguments actually
use them; name substrings and other ambiguous syntax receive manual guidance.

## Why is this bad?

Keeping behavior on the type it belongs to makes that behavior easier to discover and keeps
the module's free-function namespace focused on operations that do not belong to one type.

For example, this free function behaves like part of `Document`'s interface:

## Example

```rust
struct Document;

fn render(document: &Document) {}
```

## Use instead

Making the first parameter the receiver puts the operation where callers expect it:

```rust
struct Document;

impl Document {
    fn render(&self) {}
}
```
