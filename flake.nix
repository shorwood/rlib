{
  description = "Compiler-backed architectural lint suite for Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    dylint-src.url = "github:trailofbits/dylint/v6.0.1";
    dylint-src.flake = false;
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      dylint-src,
      ...
    }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      componentsFor =
        system:
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
            # The PyPI sdist omits fixtures and plugin tests used by nixpkgs.
            # Build the complete release source with the inherited checks intact.
            src = pkgs.fetchFromGitHub {
              owner = "sqlfluff";
              repo = "sqlfluff";
              tag = version;
              hash = "sha256:0m5y9385sb6k6pn72jf728ixcqx9abcmnl0hb5zfmcl3095lgz6r";
            };
          });
        in
        {
          inherit
            pkgs
            rust
            rustPlatform
            dylintTools
            dylintDriver
            sqlfluff
            ;
        };
      rlibPackageFor =
        system:
        let
          components = componentsFor system;
        in
        components.pkgs.callPackage ./nix/rlib.nix {
          inherit (components)
            rustPlatform
            dylintDriver
            dylintTools
            sqlfluff
            ;
          rustToolchain = components.rust.toolchain;
          toolchainLabel = components.rust.toolchainLabel;
        };
      developmentEnvironmentFor =
        system:
        let
          components = componentsFor system;
          inherit (components)
            pkgs
            rust
            dylintTools
            dylintDriver
            ;
        in
        pkgs.mkShell {
          packages = [
            rust.toolchain
            dylintTools
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

          # rlib's own linker and UI harness use Dylint's rustup-shaped
          # compatibility key. Nix still supplies both compiler and driver.
          RUSTUP_TOOLCHAIN = rust.toolchainLabel;
          DYLINT_DRIVER_PATH = "${dylintDriver}";
        };
      consumerEnvironmentFor =
        system:
        let
          components = componentsFor system;
          inherit (components) pkgs rust;
        in
        pkgs.mkShell {
          packages = [
            rust.toolchain
            (rlibPackageFor system)
          ];

          # `inputsFrom` propagates shell hooks, unlike arbitrary environment
          # attributes. Downstream shells therefore inherit the pinned
          # compiler without leaking rlib's internal Dylint compatibility key.
          shellHook = ''
            export RUSTC="${rust.toolchain}/bin/rustc"
            export RUSTDOC="${rust.toolchain}/bin/rustdoc"
            unset RUSTUP_TOOLCHAIN DYLINT_DRIVER_PATH
          '';
        };
    in
    {
      packages = forAllSystems (system: {
        default = rlibPackageFor system;
        rlib = rlibPackageFor system;
        rust-toolchain = (componentsFor system).rust.toolchain;
      });
      checks = forAllSystems (system: {
        package = self.packages.${system}.rlib;
      });
      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${rlibPackageFor system}/bin/cargo-rlib";
          meta.description = "Run the rlib lint suite";
        };
      });
      devShells = forAllSystems (system: {
        default = developmentEnvironmentFor system;
        consumer = consumerEnvironmentFor system;
      });
    };
}
