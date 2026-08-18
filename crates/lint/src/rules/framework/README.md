# Framework lints

## Summary

Rules that ask projects to choose which derive crate should generate behavior offered by several enabled libraries.

## How to enable

Enable the `framework` Cargo feature to register this family. Enable the whole family with `rlib::framework`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::framework_resolution_required`](./framework_resolution_required/README.md) | Warns when several enabled libraries can generate the same code and the project has not chosen which one to use. | Code clarity | Manual |
