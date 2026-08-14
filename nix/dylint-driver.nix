{
  cmake,
  dylintSrc,
  lib,
  makeWrapper,
  openssl,
  pkg-config,
  rustPlatform,
  rustToolchain,
  toolchainLabel,
}:

# A normal rustc process cannot load an external lint library. Dylint's driver
# is rustc with that loading step added. Building it in Nix prevents Dylint from
# downloading or preparing a second compiler behind the scenes.
rustPlatform.buildRustPackage {
  pname = "dylint-driver";
  version = "6.0.1";
  src = dylintSrc;

  # The compiler driver has its own lockfile and release cadence within the
  # Dylint source tree, so build that subproject rather than the full workspace.
  postUnpack = ''
    sourceRoot="$sourceRoot/driver"
  '';

  cargoLock.lockFile = "${dylintSrc}/driver/Cargo.lock";

  # Dylint records this name in the produced library filename. It is only a
  # compatibility identifier; the compiler used here still comes from Nix.
  RUSTUP_TOOLCHAIN = toolchainLabel;

  # Upstream exposes the driver as a library because cargo-dylint normally
  # prepares an executable on demand. A tiny entry point lets Nix prepare it
  # once with the exact compiler used by this project.
  postPatch = ''
    cat >> Cargo.toml <<'EOF'

[[bin]]
name = "dylint-driver"
path = "src/main.rs"
EOF

    cat > src/main.rs <<'EOF'
#![feature(rustc_private)]

fn main() -> anyhow::Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    dylint_driver::dylint_driver(&args)
}
EOF
  '';

  cargoBuildFlags = [ "--bin" "dylint-driver" ];
  nativeBuildInputs = [ cmake makeWrapper pkg-config ];
  buildInputs = [ openssl ];
  LD_LIBRARY_PATH = lib.makeLibraryPath [ openssl ];

  # As with the command-line tools, upstream's own test suite is outside the
  # dev environment's responsibility. The repository tests exercise this
  # driver through the actual lint UI tests.
  doCheck = false;

  # cargo-dylint searches DYLINT_DRIVER_PATH/<toolchain label>/dylint-driver.
  # Preserve that expected layout so no wrapper script or fixed path is needed
  # in the repository or editor configuration.
  installPhase = ''
    runHook preInstall
    mkdir -p "$out/${toolchainLabel}"
    find ../target/nightly -type f -path '*/release/dylint-driver' \
      -exec install -m755 {} "$out/${toolchainLabel}/.dylint-driver-unwrapped" \;

    # The driver loads rustc's private shared libraries at runtime. They live
    # beside the pinned compiler rather than in the host's library search path.
    makeWrapper "$out/${toolchainLabel}/.dylint-driver-unwrapped" \
      "$out/${toolchainLabel}/dylint-driver" \
      --prefix LD_LIBRARY_PATH : "${rustToolchain}/lib" \
      --prefix DYLD_LIBRARY_PATH : "${rustToolchain}/lib"

    # Ensure the wrapper is executable so cargo-dylint can find it. The unwrapped
    # binary is not needed by cargo-dylint, but it is useful for debugging.
    test -x "$out/${toolchainLabel}/dylint-driver"
    runHook postInstall
  '';

  meta = {
    description = "Prebuilt Dylint compiler driver";
    homepage = "https://github.com/trailofbits/dylint";
    license = with lib.licenses; [ asl20 mit ];
  };
}
