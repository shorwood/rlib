dylint-library-all:
    cargo build -p rlib-lint --all-features --locked
    host=$(rustc --version --verbose | sed -n 's/^host: //p'); case "$host" in *darwin*) extension=dylib ;; *) extension=so ;; esac; source="target/debug/librlib_lint.$extension"; target="target/debug/librlib_lint@nightly-$host.$extension"; test -f "$source"; cp "$source" "$target"

ok: dylint-library-all
    cargo check --workspace --lib --tests --all-features
    # UI examples are intentionally invalid programs; compilation, not warning cleanliness, matters.
    RUSTFLAGS="-A warnings -A duplicate_features -Zcrate-attr=feature(register_tool) -Zcrate-attr=register_tool(rlib)" cargo check --workspace --examples --all-features
    cargo check --workspace --lib --no-default-features
    cargo test --workspace --lib --tests --all-features
    # UI examples deliberately contain code that violates the rules under test.
    cargo-clippy clippy --workspace --lib --tests --all-features -- -D warnings
    DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings" cargo dylint --lib rlib_lint --workspace -- --all-features --tests
