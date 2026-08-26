{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    dylint-src.url = "github:trailofbits/dylint/v6.0.1";
    dylint-src.flake = false;
  };

  outputs = { nixpkgs, fenix, dylint-src, ... }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      componentsFor = system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          # A Dylint library is coupled to Rust compiler internals, so Rust and
          # every Dylint executable must be built from one pinned toolchain.
          rust = pkgs.callPackage ./nix/rust-toolchain.nix {
            inherit fenix system;
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rust.toolchain;
            rustc = rust.toolchain;
          };

          # Dylint comes in two parts: user-facing commands and a compiler that
          # can load our lint library. Keeping them separate makes that split
          # visible instead of hiding it in a shell hook.
          dylintTools = pkgs.callPackage ./nix/dylint-tools.nix {
            inherit rustPlatform;
            dylintSrc = dylint-src;
          };
          dylintDriver = pkgs.callPackage ./nix/dylint-driver.nix {
            inherit rustPlatform;
            dylintSrc = dylint-src;
            rustToolchain = rust.toolchain;
            toolchainLabel = rust.toolchainLabel;
          };
          sqlfluff = pkgs.sqlfluff.overridePythonAttrs (_old: rec {
            version = "4.3.0";
            src = pkgs.fetchPypi {
              pname = "sqlfluff";
              inherit version;
              hash = "sha256-qmR7NyERLxrKWB7+jqqBWjMq4hRhCglGWS+YoZ7x6Ms=";
            };
          });
        in
          {
            inherit pkgs rust rustPlatform dylintTools dylintDriver sqlfluff;
          };
      rlibPackageFor = system:
        let
          components = componentsFor system;
        in
          components.pkgs.callPackage ./nix/rlib.nix {
            inherit (components) rustPlatform dylintDriver sqlfluff;
            rustToolchain = components.rust.toolchain;
            toolchainLabel = components.rust.toolchainLabel;
          };
      devEnvironmentFor = system:
        let
          components = componentsFor system;
          inherit (components) pkgs rust dylintTools dylintDriver;
          rlib = rlibPackageFor system;
        in
          pkgs.mkShell {
            packages = [
              rust.toolchain
              dylintTools
              rlib
              pkgs.just
              pkgs.openssl
              pkgs.pkg-config
              pkgs.stdenv.cc
            ];

            # Pin compiler discovery as well as PATH lookup. This prevents
            # Cargo, Clippy, rust-analyzer, or rustdoc from silently falling
            # back to a compiler installed on the host.
            RUSTC = "${rust.toolchain}/bin/rustc";
            RUSTDOC = "${rust.toolchain}/bin/rustdoc";

            # Dylint uses a rustup-style name as a compatibility key when it
            # chooses a compiler driver. The value does not invoke rustup; the
            # driver itself comes from the Nix path below.
            RUSTUP_TOOLCHAIN = rust.toolchainLabel;
            DYLINT_DRIVER_PATH = "${dylintDriver}";
          };
    in {
      packages = forAllSystems (system: {
        default = rlibPackageFor system;
        rlib = rlibPackageFor system;
      });
      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${rlibPackageFor system}/bin/cargo-rlib";
        };
      });
      devShells = forAllSystems (system: {
        default = devEnvironmentFor system;
      });
    };
}
