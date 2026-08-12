# mixed_module_file_layouts

## What it does

Checks for a module stored in `module.rs` when a neighboring `module/` directory contains only
documentation or other non-Rust companion files. A directory containing Rust source is a normal
submodule layout and is allowed. Modules using `#[path]` are also left alone.

## Why is this bad?

Splitting one module between a flat source file and a same-name directory makes its contents harder
to find. Keeping the module root with its supporting files gives the module one obvious home.

## Example

```text
src/
├── parser.rs
└── parser/
    └── README.md
```

```rust
mod parser;
```

## Use instead

```text
src/
└── parser/
    ├── mod.rs
    └── README.md
```

```rust
mod parser;
```
