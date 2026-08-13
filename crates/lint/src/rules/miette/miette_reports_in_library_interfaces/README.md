# miette_reports_in_library_interfaces

## What it does

Finds public APIs in library crate types that return `miette::Report` directly or through
`miette::Result`.

## Why is this bad?

Reports belong at application rendering boundaries. Returning one from a library erases the
concrete failure vocabulary callers need for classification and recovery.

## Example

```rust,ignore
pub fn load() -> miette::Result<()> { todo!() }
```

## Use instead

```rust,ignore
pub fn load() -> Result<(), LoadError> { todo!() }
```
