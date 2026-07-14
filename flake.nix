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
        MedievalDependencies = with pkgs; [
         # System libraries required by dependencies go here (e.g. openssl, pkg-config)
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
            # bare rust
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer

	    # other deps
	    clang-tools
	    libclang
	    jre
	    llvmPackages.libclang
	    pkg-config
	    python3

	    openssl
	    z3
          ] ++ MedievalDependencies;

          # Fixes rust-analyzer looking for standard library source code
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";
          shellHook = ''
            export PATH="${pkgs.clangStdenv.cc}/bin:${conjure}:$PATH";
          '';
        };
      });
}

