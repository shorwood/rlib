# mixed_module_file_layouts

## What it does

Checks for every conventionally loaded module stored in `module.rs` beside a same-name `module/`
directory. Directory contents do not matter: Rust children, documentation, generated artifacts,
and empty directories all create a second physical root. Inline, macro-generated, non-local, and
explicit `#[path]` modules are outside this conventional-layout policy.

Rust 2024 accepts a flat module parent with children under its same-name directory. This lint
deliberately applies the stricter project convention that one module has one physical source root.

## Why is this bad?

Splitting one module between a flat source file and a same-name directory makes its contents harder
to find. Keeping the module root with its supporting files gives the module one obvious home.

## Example

```text
src/
├── parser.rs
└── parser/
    └── grammar.rs
```

```rust
// parser.rs
mod grammar;

// lib.rs
mod parser;
```

## Use instead

```text
src/
└── parser/
    ├── grammar.rs
    └── mod.rs
```

```rust
// parser/mod.rs
mod grammar;

// lib.rs
mod parser;
```
