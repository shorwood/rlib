# unnecessarily_broad_visibility

## What it does

Finds authored module items, types, functions, constants, statics, struct and union fields,
and inherent associated items whose canonical visibility is broader than every resolved use
in the current crate. It recommends private visibility for uses confined to the defining
module, `pub(super)` for the immediate parent subtree, and `pub(crate)` for wider crate use.

Unrestricted `pub` APIs in publishable library packages are preserved because downstream
users cannot be observed. Binaries and packages explicitly marked `publish = false` are
treated as closed. Missing or malformed manifest metadata is interpreted conservatively.
Authored reach is capped by restricted ancestor modules, so `pub` inside a private namespace
is accepted when that namespace already supplies the narrowest effective boundary. Generated
declarations, foreign entry points, proc macros, and language items are ignored.

## Why is this bad?

Broad visibility is a dependency permission, not decoration. An unnecessarily visible item
allows code to bypass its owning module, grows the compatibility surface, and makes later
relocation or invariant enforcement harder. Generated code often writes `pub` before it has
decided where an operation belongs; the resulting API then outlives that accident.

## Example

```rust
mod normalization {
    pub fn normalize_internal_key(key: &str) -> String {
        key.trim().to_owned()
    }
}
```

## Use instead


If every caller is inside `normalization`, keep ownership local:

```rust
mod normalization {
    fn normalize_internal_key(key: &str) -> String {
        key.trim().to_owned()
    }
}
```

Suggestions are deliberately `MaybeIncorrect`: inactive `cfg` branches and Rust consumers
outside a closed workspace may be invisible to the current compiler invocation.
