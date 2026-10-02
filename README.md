# rlib

rlib is a collection of compiler-backed lints for Rust architecture, API design, and ecosystem
crates. It runs as a Cargo subcommand and includes dedicated lint families for libraries such as
Axum, Serde, SQLx, Strum, Leptos, and thiserror.

See the [lint catalog](crates/lint/src/rules/README.md) for the available rules.

## Installation

Nix is the supported installation method. Install the 1.0.0 release directly from GitHub:

```console
nix profile install github:shorwood/rlib/v1.0.0
```

Use `.` instead of the GitHub reference when installing from a checkout.

The package bundles the matching Rust toolchain, compiler driver, lint library, and SQLFluff
backend.

## Usage

Run rlib like any other Cargo check command:

```console
cargo rlib
cargo rlib --workspace --all-targets --all-features
cargo rlib --fix
cargo rlib --workspace -- -D warnings
```

Arguments before `--` are passed to Cargo; arguments after it are passed to rustc. `--fix` applies
machine-applicable native suggestions and then checks the project again.

Lint levels can also be set in Rust code:

```rust
#![warn(rlib::all)]
#![deny(rlib::sqlx_unchecked_query_macros)]
```

## SQLx and SQLFluff

Native SQLx lints run with the rest of rlib. SQLFluff analysis is enabled for queries covered by a
project SQLFluff configuration. To create one:

```console
cargo rlib sqlx init --dialect sqlite
```

`postgres`, `mysql`, and `sqlite` are supported.

## Development

```console
nix develop
just ok
```

## Releasing

Stable releases use `vX.Y.Z` tags matching the workspace version. Run `just ok`
and `nix flake check`, push `main`, wait for CI, then tag that commit and create
the GitHub Release.

## License

rlib is available under the [MIT License](LICENSE).
