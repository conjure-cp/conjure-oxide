#!/usr/bin/env bash
set -euo pipefail

# Setup script for running conjure-oxide experiments.
# Everything is installed locally under .local/ in the repo root.
#
# What it does:
#   1. Installs rustup + the toolchain version from rust-toolchain.toml (if missing)
#   2. Initialises git submodules (e.g. minion)
#   3. Builds z3 from source (static library, needed by z3-sys / conjure-oxide)
#   4. Builds conjure-oxide in release mode
#   5. Downloads conjure + savilerow from GitHub releases, builds minion from source
#   6. Creates a Python venv with experiment + analysis dependencies
#
# Usage:
#   cd conjure-oxide
#   bash diss/setup-server.sh
#
# After setup, activate the environment with:
#   source diss/activate.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOCAL="$REPO_ROOT/diss/.local"
CONJURE_VERSION="v2.6.0"
CONJURE_URL="https://github.com/conjure-cp/conjure/releases/download/${CONJURE_VERSION}/conjure-${CONJURE_VERSION}-linux-with-solvers.zip"
Z3_VERSION="4.13.4"
PYTHON_MIN_VERSION="3.10"

mkdir -p "$LOCAL"

# -- helpers -----------------------------------------------------------------

log() { echo ">>> $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }

check_python() {
    # find a python3 >= PYTHON_MIN_VERSION
    for candidate in python3.12 python3.11 python3.10 python3; do
        if command -v "$candidate" &>/dev/null; then
            local ver
            ver=$("$candidate" -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
            if "$candidate" -c "
import sys
min_ver = tuple(int(x) for x in '$PYTHON_MIN_VERSION'.split('.'))
sys.exit(0 if sys.version_info[:2] >= min_ver else 1)
"; then
                PYTHON="$candidate"
                log "Found $PYTHON (version $ver)"
                return 0
            fi
        fi
    done
    die "No Python >= $PYTHON_MIN_VERSION found on PATH. Install one or load a module (e.g. 'module load python/3.11')."
}

check_cmake() {
    if ! command -v cmake &>/dev/null; then
        die "cmake is required to build z3 from source but was not found on PATH."
    fi
    log "Found cmake: $(cmake --version | head -1)"
}

# -- 1. Rust toolchain -------------------------------------------------------

log "Checking Rust toolchain..."

if ! command -v rustup &>/dev/null; then
    log "rustup not found, installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# rust-toolchain.toml tells rustup which version to use automatically,
# but let's make sure the toolchain is actually installed
RUST_CHANNEL=$(grep '^channel' "$REPO_ROOT/rust-toolchain.toml" | sed 's/.*= *"\(.*\)"/\1/')
log "Ensuring Rust toolchain $RUST_CHANNEL is installed..."
rustup toolchain install "$RUST_CHANNEL" --profile minimal
rustup default "$RUST_CHANNEL"

log "Rust: $(rustc --version), Cargo: $(cargo --version)"

# -- 2. Initialise submodules ------------------------------------------------

log "Initialising git submodules..."
cd "$REPO_ROOT"
git submodule sync --recursive
git submodule update --init --recursive

# -- 3. Install z3 locally (built from source) --------------------------------

Z3_DIR="$LOCAL/z3"

if [ -f "$Z3_DIR/include/z3.h" ] && { [ -f "$Z3_DIR/lib/libz3.a" ] || [ -f "$Z3_DIR/lib64/libz3.a" ]; }; then
    log "z3 already installed at $Z3_DIR"
else
    check_cmake

    Z3_SRC="$LOCAL/z3-src"
    Z3_TAG="z3-${Z3_VERSION}"

    if [ -d "$Z3_SRC/.git" ]; then
        log "z3 source already cloned at $Z3_SRC"
    else
        log "Cloning z3 ${Z3_VERSION} source..."
        rm -rf "$Z3_SRC"
        git clone --depth 1 --branch "$Z3_TAG" https://github.com/Z3Prover/z3.git "$Z3_SRC"
    fi

    log "Building z3 (static library, release)... this may take a while on first run."
    cmake -S "$Z3_SRC" -B "$Z3_SRC/build" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$Z3_DIR" \
        -DZ3_BUILD_LIBZ3_SHARED=OFF \
        -DZ3_BUILD_EXECUTABLE=OFF \
        -DZ3_BUILD_TEST_EXECUTABLES=OFF \
        -DZ3_BUILD_PYTHON_BINDINGS=OFF \
        -DZ3_BUILD_JAVA_BINDINGS=OFF \
        -DZ3_BUILD_DOTNET_BINDINGS=OFF

    cmake --build "$Z3_SRC/build" -j"$(nproc)" --config Release
    cmake --install "$Z3_SRC/build"

    # Some distros install to lib64/ instead of lib/; normalise to lib/
    if [ -d "$Z3_DIR/lib64" ] && [ ! -d "$Z3_DIR/lib" ]; then
        ln -s lib64 "$Z3_DIR/lib"
    fi

    test -f "$Z3_DIR/include/z3.h" || die "z3.h not found after build"
    test -f "$Z3_DIR/lib/libz3.a"  || die "libz3.a not found after build"
    log "z3 built and installed at $Z3_DIR"

    # source tree no longer needed (keep it around for re-builds; rm if disk is tight)
    # rm -rf "$Z3_SRC"
fi

# export z3 paths so cargo / z3-sys can find the header and static library
export Z3_SYS_Z3_HEADER="$Z3_DIR/include/z3.h"
export LIBRARY_PATH="$Z3_DIR/lib:$Z3_DIR/lib64${LIBRARY_PATH:+:$LIBRARY_PATH}"
export C_INCLUDE_PATH="$Z3_DIR/include${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"

log "z3: $(ls "$Z3_DIR/lib/"libz3* 2>/dev/null || echo 'libs not found')"

# -- 4. Build conjure-oxide --------------------------------------------------

log "Building conjure-oxide (release)..."
cd "$REPO_ROOT"
cargo build --release -p conjure-cp-cli --bin conjure-oxide

OXIDE_BIN="$REPO_ROOT/target/release/conjure-oxide"
test -f "$OXIDE_BIN" || die "Build succeeded but $OXIDE_BIN not found"
log "conjure-oxide built at $OXIDE_BIN"

# -- 5. Conjure + solvers ----------------------------------------------------

CONJURE_DIR="$LOCAL/conjure"

if [ -f "$CONJURE_DIR/conjure" ]; then
    log "Conjure already installed at $CONJURE_DIR"
else
    log "Downloading Conjure $CONJURE_VERSION..."
    TMPZIP=$(mktemp /tmp/conjure-XXXXXX.zip)
    curl -L -o "$TMPZIP" "$CONJURE_URL"

    log "Extracting..."
    TMPEXTRACT=$(mktemp -d /tmp/conjure-extract-XXXXXX)
    unzip -q "$TMPZIP" -d "$TMPEXTRACT"

    # the zip extracts into a single directory like conjure-v2.6.0-linux-with-solvers/
    INNER=$(find "$TMPEXTRACT" -maxdepth 1 -mindepth 1 -type d | head -1)
    if [ -z "$INNER" ]; then
        # no inner directory -- files are at top level
        INNER="$TMPEXTRACT"
    fi

    rm -rf "$CONJURE_DIR"
    mv "$INNER" "$CONJURE_DIR"
    rm -f "$TMPZIP"
    rm -rf "$TMPEXTRACT"

    chmod +x "$CONJURE_DIR/conjure" "$CONJURE_DIR/savilerow" "$CONJURE_DIR/minion" 2>/dev/null || true
    log "Conjure installed at $CONJURE_DIR"
fi

# sanity check
"$CONJURE_DIR/conjure" --version || die "conjure binary doesn't run"

# -- 5b. Build minion from source (replace pre-built binary) -----------------
# The pre-built minion in the Conjure release may require a newer glibc than
# the server provides. Build from the repo submodule source to guarantee
# compatibility.

MINION_SRC="$REPO_ROOT/crates/minion-sys/vendor"
MINION_MARKER="$CONJURE_DIR/.minion-built-from-source"

if [ -f "$MINION_MARKER" ]; then
    log "minion already built from source"
else
    log "Building minion from source (standalone executable)..."
    MINION_BUILD="$LOCAL/minion-build"
    mkdir -p "$MINION_BUILD"
    cd "$MINION_BUILD"
    python3 "$MINION_SRC/configure.py" --quick
    make -j"$(nproc)"

    # The built binary is at $MINION_BUILD/minion
    test -f "$MINION_BUILD/minion" || die "minion binary not found after build"

    # Replace the pre-built minion in the Conjure directory
    cp "$MINION_BUILD/minion" "$CONJURE_DIR/minion"
    chmod +x "$CONJURE_DIR/minion"
    touch "$MINION_MARKER"
    log "minion built from source and installed at $CONJURE_DIR/minion"
fi

# sanity check
"$CONJURE_DIR/minion" --help > /dev/null 2>&1 || die "minion binary doesn't run"
log "minion: $("$CONJURE_DIR/minion" --help 2>&1 | head -1)"

# -- 6. Python venv -----------------------------------------------------------

check_python

VENV="$LOCAL/venv"

if [ -d "$VENV" ]; then
    log "Python venv already exists at $VENV"
else
    log "Creating Python venv..."
    "$PYTHON" -m venv "$VENV"
fi

# activate it for the rest of this script
source "$VENV/bin/activate"

log "Installing Python packages..."
pip install --upgrade pip setuptools wheel -q
pip install \
    pandas \
    matplotlib \
    seaborn \
    scipy \
    numpy \
    jupyter \
    -q

log "Python venv ready at $VENV"
log "Python: $(python3 --version), pip: $(pip --version | cut -d' ' -f1-2)"

# -- 7. Write activation script ----------------------------------------------

ACTIVATE_SCRIPT="$REPO_ROOT/diss/activate.sh"
cat > "$ACTIVATE_SCRIPT" << 'ACTIVATE_EOF'
#!/usr/bin/env bash
# Source this to set up the experiment environment:
#   source diss/activate.sh

_DISS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_REPO_ROOT="$(cd "$_DISS_DIR/.." && pwd)"
_LOCAL="$_DISS_DIR/.local"
_CONJURE_DIR="$_LOCAL/conjure"
_Z3_DIR="$_LOCAL/z3"

# conjure + solvers
export PATH="$_CONJURE_DIR:$PATH"
export LD_LIBRARY_PATH="${_CONJURE_DIR}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# z3 (for building conjure-oxide / z3-sys – static linking only)
export Z3_SYS_Z3_HEADER="$_Z3_DIR/include/z3.h"
export LIBRARY_PATH="$_Z3_DIR/lib:$_Z3_DIR/lib64${LIBRARY_PATH:+:$LIBRARY_PATH}"
export C_INCLUDE_PATH="$_Z3_DIR/include${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"

# cargo / rust
export PATH="$HOME/.cargo/bin:$PATH"

# python venv
source "$_LOCAL/venv/bin/activate"

# conjure-oxide binary
export PATH="$_REPO_ROOT/target/release:$PATH"

echo "Environment ready."
echo "  conjure:       $(which conjure)"
echo "  conjure-oxide: $(which conjure-oxide)"
echo "  python:        $(which python3) ($(python3 --version))"
echo "  minion:        $(which minion 2>/dev/null || echo 'not found')"
ACTIVATE_EOF

chmod +x "$ACTIVATE_SCRIPT"

# -- done --------------------------------------------------------------------

log ""
log "Setup complete!"
log ""
log "To use the environment:"
log "  source diss/activate.sh"
log ""
log "Then run experiments, e.g.:"
log "  cd diss/experiments/scaling-models"
log "  python3 run_all.py --runs 3 --threads 4"

