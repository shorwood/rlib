# foreign_type_method_like_free_functions

## What it does

Checks visible free functions that take foreign named types and recommends a focused
extension trait when one foreign parameter remains the clear behavioral subject. Repeated
foreign dependencies are treated as global infrastructure only when they occur in at
least two functions beside at least two different named co-parameters. Functions with a
local named input or a local direct, optional, or fallible success return are left to that
stronger local owner instead. Functions whose foreign inputs are all proven ambient
infrastructure have no extension-trait subject and are also left alone.

## Why is this bad?

A public helper namespace hides which operations belong together and separates behavior
from the type callers already use to discover it. A focused extension trait keeps the
behavior colocated without pretending the foreign type itself can gain inherent methods.

## Example

```rust
use std::path::Path;

pub fn is_project_manifest(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "Cargo.toml")
}
```

## Use instead

Put the operation behind the behavioral subject instead:

```rust
use std::path::Path;

trait ProjectPathExt {
    fn is_project_manifest(&self) -> bool;
}

impl ProjectPathExt for Path {
    fn is_project_manifest(&self) -> bool {
        self.file_name().is_some_and(|name| name == "Cargo.toml")
    }
}
```

When several foreign parameters remain plausible, the lint still reports the public helper
but asks the author to choose an explicit owner instead of guessing.
