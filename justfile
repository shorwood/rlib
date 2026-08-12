ok:
    cargo check --workspace --all-targets --all-features
    cargo check --workspace --lib --no-default-features
    cargo test --workspace --all-features
    cargo-clippy clippy --workspace --all-targets --all-features -- -D warnings
    DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings" cargo dylint --lib rlib_lint --workspace -- --all-features
