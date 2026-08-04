ok:
    cargo check --workspace --all-targets
    cargo test --workspace
    cargo-clippy clippy --workspace --all-targets
    DYLINT_LIBRARY_PATH="$PWD/target/debug" cargo dylint --lib rlib_lint --workspace
