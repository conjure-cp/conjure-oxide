{
  description = "A basic Nix Flake for Rust development";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, utils, rust-overlay, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        # Dependencies for development that are not system packages, but still required for development (eg; z3 and JDK)
        DevDependencies = with pkgs; [
          python3
          jre
          z3
        ];

        # System libraries go here (e.g. openssl, pkg-config)
        MedievalDependencies = with pkgs; [
          clang-tools
          libclang
          llvmPackages.libclang
          pkg-config
          openssl
        ];

        # rust-specific dependencies
        RustDependencies = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          rust-analyzer
        ];

        clangMkShell = pkgs.mkShell.override { stdenv = pkgs.clangStdenv; };

        conjure = pkgs.stdenv.mkDerivation rec {
          pname = "conjure";
          version = "nightly";

          src = pkgs.fetchzip {
            url = "https://github.com/conjure-cp/conjure/releases/download/nightly/conjure-nightly-linux-with-solvers.zip";
            # Replace with the hash `nix build` reports on first run.
            sha256 = "sha256-5G9id0dYRqL56RMNbM6vWwE5lev62prtN4a18N4pNtI=";
          };

          nativeBuildInputs = [ pkgs.autoPatchelfHook ];
          buildInputs = with pkgs; [
            stdenv.cc.cc.lib # libstdc++, libgcc_s
            zlib
            gmp
            bzip2
            numactl
          ];

          dontBuild = true;

          installPhase = ''
            runHook preInstall
            mkdir -p $out
            cp -r . $out/
            runHook postInstall
          '';

          meta = with pkgs.lib; {
            description = "Conjure: The Automated Constraint Modelling Tool (nightly build, with bundled solvers)";
            homepage = "https://github.com/conjure-cp/conjure";
            platforms = [ "x86_64-linux" ];
          };
        };

      in
      {
        devShells.default = clangMkShell {
          buildInputs = with pkgs; [
          ] ++ MedievalDependencies ++ DevDependencies ++ RustDependencies;

          # Fixes rust-analyzer looking for standard library source code
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";
          shellHook = ''
            export PATH="${pkgs.clangStdenv.cc}/bin:${conjure}:$PATH";
          '';
        };
      });
}

