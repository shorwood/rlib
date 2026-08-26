# cargo rlib

`cargo rlib` runs the complete compiler-backed rlib analysis suite with Cargo-compatible target
selection and diagnostics:

```console
cargo rlib --workspace --all-targets --all-features
cargo rlib --fix --package application
cargo rlib --workspace -- -D warnings
```

The Nix package is the supported installation unit. It bundles the pinned Rust compiler, Dylint
driver, matching rlib lint library, and SQLFluff backend; projects do not configure or install
Dylint or SQLFluff separately. Use `nix run . -- --version`, or install `packages.default` so Cargo
can discover the `cargo-rlib` executable.

Native rlib lints always run. SQL analysis is enabled per query only when an applicable project
SQLFluff configuration exists. Create an initial project-owned configuration with:

```console
cargo rlib sqlx init --dialect sqlite
```

The initializer supports `postgres`, `mysql`, and `sqlite`, refuses to overwrite existing policy,
and writes `.sqlfluff`. SQL backend diagnostics are reported as `rlib::sql::<rule>` and participate
in Cargo human, short, and JSON output. `--fix` applies compiler-native machine-applicable
suggestions, then reruns analysis; SQL findings are always diagnostic-only.
