# `rlib::mixed_module_file_layouts`

## Summary

Checks for every conventionally loaded module stored in `module.rs` beside a same-name `module/` directory.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for every conventionally loaded module stored in `module.rs` beside a same-name `module/`
directory. Directory contents do not matter: Rust children, documentation, generated artifacts,
and empty directories all create a second physical root. Inline, macro-generated, non-local, and
explicit `#[path]` modules are outside this conventional-layout policy.

Rust 2024 accepts a flat module parent with children under its same-name directory. This lint
deliberately applies the stricter project convention that one module has one physical source root.

## Why this matters

Splitting one module between a flat source file and a same-name directory makes its contents harder
to find. Keeping the module root with its supporting files gives the module one obvious home.

## Examples

### Triggers the lint

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

### Use this instead

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

## What it skips

Inline modules, macro-generated modules, non-local modules, and modules with an explicit `#[path]` are not checked.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::invalid_barrel_file_items`](../invalid_barrel_file_items/README.md) — Keeps module-root files focused on module declarations.
