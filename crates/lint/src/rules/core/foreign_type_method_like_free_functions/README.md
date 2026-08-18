# `rlib::foreign_type_method_like_free_functions`

## Summary

Checks visible free functions that take foreign named types and recommends a focused extension trait when one foreign parameter remains the clear behavioral subject.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks visible free functions that take foreign named types and recommends a focused
extension trait when one foreign parameter remains the clear behavioral subject. Repeated
foreign dependencies are treated as global infrastructure only when they occur in at
least two functions beside at least two different named co-parameters. Functions with a
local named input or a local direct, optional, or fallible success return are left to that
stronger local owner instead. Functions whose foreign inputs are all proven ambient
infrastructure have no extension-trait subject and are also left alone.

## Why this matters

A public helper namespace hides which operations belong together and separates behavior
from the type callers already use to discover it. A focused extension trait keeps the
behavior colocated without pretending the foreign type itself can gain inherent methods.

## Examples

### Triggers the lint

```rust
use std::path::Path;

pub fn is_project_manifest(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "Cargo.toml")
}
```

### Use this instead

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

## What it skips

Repeated foreign dependencies are treated as global infrastructure only when they occur in at least two functions beside at least two different named co-parameters. Functions whose foreign inputs are all proven ambient infrastructure have no extension-trait subject and are also left alone.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
