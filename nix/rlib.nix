{
  lib,
  makeWrapper,
  rustPlatform,
  rustToolchain,
  toolchainLabel,
  dylintDriver,
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
  cargoTestFlags = [
    "--workspace"
    "--lib"
    "--tests"
    "--all-features"
  ];

  nativeBuildInputs = [ makeWrapper ];
  preCheck = ''
    export RUSTUP_TOOLCHAIN="${toolchainLabel}"
    export DYLINT_DRIVER_PATH="${dylintDriver}"
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
      install -m755 "target/release/cargo-rlib" \
        "$out/libexec/rlib/cargo-rlib"
      install -m755 "target/release/librlib_lint.${extension}" \
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
