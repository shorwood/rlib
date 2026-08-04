{ fenix, lib, system }:

# One Rust installation serves the terminal, editor, and Dylint. This avoids a
# common failure mode where each tool quietly chooses a different compiler.
let
  # Dylint includes the host target in its driver key. Derive that key from
  # the flake system instead of asking rustup, which is intentionally absent.
  targetTriple = {
    x86_64-linux = "x86_64-unknown-linux-gnu";
    aarch64-linux = "aarch64-unknown-linux-gnu";
    x86_64-darwin = "x86_64-apple-darwin";
    aarch64-darwin = "aarch64-apple-darwin";
  }.${system} or (throw "Unsupported system: ${system}");

  # Custom lints use unstable rustc APIs. Even nearby nightly releases can
  # change those APIs, so one date is shared by the compiler, Dylint, and Zed.
  datedToolchain = fenix.packages.${system}.toolchainOf {
    channel = "nightly";
    date = "2026-04-16";
    sha256 = "1kbgfwz1hd2z6fkq97650in4fwa13y9n6rhjankh5hbmis8mqkn3";
  };
in {
  toolchain = datedToolchain.withComponents [
    # Everyday Cargo development.
    "cargo"
    "clippy"
    "rustfmt"

    # Editor support comes from the same environment as the compiler. Zed can
    # therefore discover it through direnv without a machine-specific path.
    "rust-analyzer"
    "rust-src"

    # Dylint links directly to compiler internals and needs their development
    # libraries. llvm-tools-preview supplies the matching LLVM utilities.
    "llvm-tools-preview"
    "rustc"
    "rustc-dev"
  ];
  inherit targetTriple;

  # This looks like a rustup name because Dylint uses that naming convention
  # as an identifier. Nix still owns and supplies the actual toolchain.
  toolchainLabel = "nightly-${targetTriple}";
}
