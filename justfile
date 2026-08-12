ok:
    cargo check --workspace --all-targets
    cargo check --workspace --lib --no-default-features
    cargo test --workspace
    cargo-clippy clippy --workspace --all-targets -- -D warnings
    DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings" cargo dylint --lib rlib_lint --workspace
