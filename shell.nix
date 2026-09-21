{ pkgs ? import <nixpkgs> {} }:
let

    clangMkShell = pkgs.mkShell.override { stdenv = pkgs.clangStdenv; };

    # Dependencies for development that are not system packages, but still required for development (eg; z3 and JDK)
    DevDependencies = with pkgs; [
        python3
        jre
        z3
        pkgs.mdbook
        git
        gh
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

    conjure = pkgs.stdenv.mkDerivation rec {
        pname = "conjure";
        version = "v2.6.1";
        zipfile = "conjure-v2.6.1-linux-with-solvers.zip";

        src = pkgs.fetchzip {
            # NOTE: here, there is some scope for changes in future
            # we can use the following line in order to always use the latest release
            # url = "https://github.com/conjure-cp/conjure/releases/latest/conjure-nightly-linux-with-solvers.zip";
            # However, this is not `nix-like' because the source is not immutable.

            url = "https://github.com/conjure-cp/conjure/releases/download/" + version + "/" + zipfile;
            # Replace with the hash `nix build` reports on first run.
            sha256 = "sha256-+pc634fz3cqsFuXiUbtj/jrMPbGZMrye7pANNaw+ejE=";
        };

        nativeBuildInputs = [ pkgs.autoPatchelfHook ];
        buildInputs = with pkgs; [
            stdenv.cc.cc.lib
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

clangMkShell {
    buildInputs = with pkgs; [
    ] ++ MedievalDependencies ++ DevDependencies ++ RustDependencies;

    # Fixes rust-analyzer looking for standard library source code
    RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
    LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";

    shellHook = ''
      export PATH="${pkgs.clangStdenv.cc}/bin:${conjure}:$PATH";
    '';
}
