ok:
    cargo check --workspace --lib --tests --all-features
    # UI examples are intentionally invalid programs; compilation, not warning cleanliness, matters.
    RUSTFLAGS="-A warnings" cargo check --workspace --examples --all-features
    cargo check --workspace --lib --no-default-features
    cargo test --workspace --lib --tests --all-features
    # UI examples deliberately contain code that violates the rules under test.
    cargo-clippy clippy --workspace --lib --tests --all-features -- -D warnings
    # Core policy work is intentionally outside this framework-lint validation slice.
    DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings -A rlib_core" cargo dylint --lib rlib_lint --workspace -- --all-features
