{
  lib,
  makeWrapper,
  openssl,
  pkg-config,
  rustPlatform,
  rustToolchain,
  toolchainLabel,
  dylintDriver,
  dylintTools,
  sqlfluff,
  stdenv,
}:

rustPlatform.buildRustPackage {
  pname = "rlib";
  version = "0.1.0";
  src = lib.cleanSource ../.;

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [
    "-p"
    "rlib-cli"
    "-p"
    "rlib-lint"
    "--all-features"
  ];

  nativeBuildInputs = [
    makeWrapper
    pkg-config
    dylintTools
  ];
  buildInputs = [ openssl ];
  RUSTC = "${rustToolchain}/bin/rustc";
  RUSTDOC = "${rustToolchain}/bin/rustdoc";
  RUSTUP_TOOLCHAIN = toolchainLabel;

  # Dylint distinguishes registry crates from its source checkout by checking
  # whether their manifest lives under CARGO_HOME. Keep Nix's vendored crates
  # there so it uses the separately packaged driver instead of ../driver.
  postPatch = ''
    : "''${CARGO_HOME:=$NIX_BUILD_TOP/cargo-home}"
    export CARGO_HOME
    mkdir -p "$CARGO_HOME"
    mv "$cargoDepsCopy" "$CARGO_HOME/vendor"
    substituteInPlace "$NIX_BUILD_TOP/.cargo/config.toml" \
      --replace-fail 'directory = "cargo-vendor-dir"' "directory = \"$CARGO_HOME/vendor\""
    export cargoDepsCopy="$CARGO_HOME/vendor"
  '';
  preCheck = ''
    # Dylint recovers compiler flags from Cargo's verbose rustc invocation;
    # the cargo-auditable wrapper obscures that invocation during UI tests.
    export PATH="${rustToolchain}/bin:$PATH"
    export DYLINT_DRIVER_PATH="${dylintDriver}"
  '';

  # Dylint's UI harness builds and loads native libraries from target/debug.
  # The standard hook exports CARGO_BUILD_TARGET to nested Cargo processes,
  # which moves those libraries into target/<triple>/debug instead.
  checkPhase = ''
    runHook preCheck
    cargo test --offline -j "$NIX_BUILD_CORES" --release \
      --target ${stdenv.hostPlatform.rust.rustcTarget} \
      --workspace --lib --tests --all-features
    runHook postCheck
  '';

  installPhase =
    let
      extension = if stdenv.isDarwin then "dylib" else "so";
      host = stdenv.hostPlatform.config;
      libraryName = "librlib_lint@nightly-${host}.${extension}";
    in
    ''
      runHook preInstall
      mkdir -p "$out/bin" "$out/lib/rlib" "$out/libexec/rlib"
      install -m755 "target/${stdenv.hostPlatform.rust.rustcTarget}/release/cargo-rlib" \
        "$out/libexec/rlib/cargo-rlib"
      install -m755 "target/${stdenv.hostPlatform.rust.rustcTarget}/release/librlib_lint.${extension}" \
        "$out/lib/rlib/${libraryName}"
      makeWrapper "$out/libexec/rlib/cargo-rlib" "$out/bin/cargo-rlib" \
        --prefix PATH : "${rustToolchain}/bin" \
        --set RUSTC "${rustToolchain}/bin/rustc" \
        --set RUSTDOC "${rustToolchain}/bin/rustdoc" \
        --set RUSTUP_TOOLCHAIN "${toolchainLabel}" \
        --set DYLINT_DRIVER_PATH "${dylintDriver}" \
        --set RLIB_INTERNAL_LINT_LIBRARY_PATH "$out/lib/rlib/${libraryName}" \
        --set RLIB_INTERNAL_SQLFLUFF_PATH "${sqlfluff}/bin/sqlfluff"
      runHook postInstall
    '';

  meta = {
    description = "Compiler-backed architectural lint suite for Rust";
    mainProgram = "cargo-rlib";
    platforms = lib.platforms.unix;
  };
}
