{
  cmake,
  dylintSrc,
  lib,
  openssl,
  pkg-config,
  rustPlatform,
}:

# `cargo-dylint` runs the project's lint library. `dylint-link` makes the
# library identifiable by the compiler version it was built for. Packaging
# both here means developers only need to enter the Nix shell.
rustPlatform.buildRustPackage {
  pname = "dylint-tools";
  version = "6.0.1";
  src = dylintSrc;

  cargoLock.lockFile = "${dylintSrc}/Cargo.lock";

  # Only the two executables needed during development belong in the shell.
  # Building the whole upstream workspace would add unrelated Dylint tools.
  cargoBuildFlags = [
    "-p"
    "cargo-dylint"
    "-p"
    "dylint-link"
  ];

  # Dylint itself depends on native TLS and Git libraries. Supplying them here
  # keeps their discovery inside Nix instead of relying on the host system.
  nativeBuildInputs = [ cmake pkg-config ];
  buildInputs = [ openssl ];
  LD_LIBRARY_PATH = lib.makeLibraryPath [ openssl ];

  # This derivation packages upstream tools; it does not validate rlib. The
  # project's lint and UI tests remain ordinary Cargo tests in the dev shell.
  doCheck = false;

  meta = {
    description = "Nix-packaged cargo-dylint and dylint-link";
    homepage = "https://github.com/trailofbits/dylint";
    license = with lib.licenses; [ asl20 mit ];
  };
}
