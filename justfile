ok:
    cargo check --workspace --lib --tests --all-features
    # UI examples are intentionally invalid programs; compilation, not warning cleanliness, matters.
    RUSTFLAGS="-A warnings -A duplicate_features -Zcrate-attr=feature(register_tool) -Zcrate-attr=register_tool(rlib)" cargo check --workspace --examples --all-features
    cargo check --workspace --lib --no-default-features
    cargo test --workspace --lib --tests --all-features
    # UI examples deliberately contain code that violates the rules under test.
    cargo-clippy clippy --workspace --lib --tests --all-features -- -D warnings
    cargo build -p rlib-lint --all-features
    cp target/debug/librlib_lint.so target/debug/librlib_lint@nightly-x86_64-unknown-linux-gnu.so
    DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings" cargo dylint --lib rlib_lint --workspace -- --all-features --tests
