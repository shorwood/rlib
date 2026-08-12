# foreign_type_method_like_free_functions

## What it does

Checks visible free functions that take foreign nominal types and recommends a focused
extension trait when one foreign parameter remains the clear semantic subject. Repeated
foreign dependencies are treated as ambient infrastructure only when they occur in at
least two functions beside at least two different nominal co-parameters. Functions with a
local nominal input or a local direct, optional, or fallible success return are left to that
stronger local owner instead.

## Why is this bad?

A public helper namespace hides which operations belong together and separates behavior
from the type callers already use to discover it. A focused extension trait keeps the
behavior colocated without pretending the foreign type itself can gain inherent methods.

## Example

```rust
pub fn direct_struct(cx: &LateContext<'_>, item: &Item<'_>) -> Option<LocalDefId> {
    // ...
}
```

## Use instead


Put the operation behind the semantic subject instead:

```rust
trait ItemExt {
    fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId>;
}

impl ItemExt for Item<'_> {
    fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId> {
        // ...
    }
}
```

When several foreign parameters remain plausible, the lint still reports the public helper
but asks the author to choose an explicit owner instead of guessing.
