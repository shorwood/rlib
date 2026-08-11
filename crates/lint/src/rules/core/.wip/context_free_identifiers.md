# Context-free identifiers

## Proposition

Add a `context_free_identifiers` lint for semantically important declarations whose names do not
identify their role.

```rust
// Bad: every name delegates meaning to surrounding code.
fn process(data: Data) -> Result<Output> {
    let result = handle(data)?;
    transform(result)
}

// Better: the dataflow can be followed without opening each callee.
fn compile_manifest(manifest: Manifest) -> Result<BuildPlan> {
    let resolved_packages = resolve_dependencies(manifest)?;
    assemble_build_plan(resolved_packages)
}
```

## Conservative detection

- Restrict diagnostics to functions, types, modules, fields, and bindings with a nontrivial live
  range.
- Use a configurable vocabulary of weak stems such as `data`, `info`, `item`, `manager`,
  `processor`, `result`, `stuff`, and `utils`.
- Consider surrounding type and function names before deciding that a short name lacks context.
- Avoid diagnostics for conventional, tightly scoped iterator and closure bindings.

```rust
records.iter().map(|item| item.name())
```

## Open decisions

- Whether vocabulary should be fixed, configurable, or inferred from structural evidence.
- How many source lines make a local binding semantically important.
- Whether generic words are acceptable when paired with a precise type.
