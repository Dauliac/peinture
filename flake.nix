{
  description = "peinture - Terminal UI component library with design tokens and layered rendering";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} ({...}: {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];

      perSystem = {
        system,
        pkgs,
        ...
      }: let
        overlays = [(import inputs.rust-overlay)];
        pkgsWithOverlay = import inputs.nixpkgs {inherit system overlays;};
        rustToolchain = pkgsWithOverlay.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-analyzer" "clippy" "rustfmt"];
        };
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource ./.;
        commonArgs = {
          inherit src;
          pname = "peinture";
          version = "0.1.0";
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        packages = {
          default = craneLib.buildPackage (commonArgs
            // {
              inherit cargoArtifacts;
              doCheck = false;
            });
          peinture-clippy = craneLib.cargoClippy (commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            });
          peinture-test = craneLib.cargoNextest (commonArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = [pkgsWithOverlay.cargo-nextest];
            });
          peinture-doc = craneLib.cargoDoc (commonArgs
            // {
              inherit cargoArtifacts;
              RUSTDOCFLAGS = "-D warnings";
            });
        };

        checks = {
          inherit (pkgsWithOverlay) statix;
          clippy = craneLib.cargoClippy (commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            });
          test = craneLib.cargoNextest (commonArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = [pkgsWithOverlay.cargo-nextest];
            });
        };

        devShells.default = craneLib.devShell {
          packages = with pkgsWithOverlay; [
            rustToolchain
            cargo-nextest
            cargo-watch
            cargo-machete
            bacon
          ];
        };
      };
    });
}
