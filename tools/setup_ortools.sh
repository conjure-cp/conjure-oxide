#!/usr/bin/env bash
set -e

# Move to the root directory of the project
cd "$(dirname "$0")/.."

if [ -f ".ortools/include/ortools/base/base_export.h" ]; then
    echo "OR-Tools is already installed locally in .ortools/"
    exit 0
fi

echo "Setting up local OR-Tools..."

TAG="v9.11"
VERSION="9.11.4210"

OS=$(uname -s)
ARCH=$(uname -m)

if [ "$OS" = "Linux" ]; then
    # We use Ubuntu 24.04 binaries for Linux as they are generally compatible with modern distros
    URL="https://github.com/google/or-tools/releases/download/${TAG}/or-tools_amd64_ubuntu-24.04_cpp_v${VERSION}.tar.gz"
elif [ "$OS" = "Darwin" ]; then
    if [ "$ARCH" = "arm64" ]; then
        URL="https://github.com/google/or-tools/releases/download/${TAG}/or-tools_arm64_macOS-14.5_cpp_v${VERSION}.tar.gz"
    else
        URL="https://github.com/google/or-tools/releases/download/${TAG}/or-tools_x86_64_macOS-14.5_cpp_v${VERSION}.tar.gz"
    fi
else
    echo "Unsupported OS: $OS"
    exit 1
fi

echo "Downloading OR-Tools from ${URL}..."
mkdir -p .ortools_tmp
if command -v curl >/dev/null 2>&1; then
    curl -L -s "${URL}" -o .ortools_tmp/ortools.tar.gz
elif command -v wget >/dev/null 2>&1; then
    wget -q "${URL}" -O .ortools_tmp/ortools.tar.gz
else
    echo "Error: curl or wget is required to download OR-Tools."
    exit 1
fi

echo "Extracting..."
rm -rf .ortools
mkdir -p .ortools
tar -xzf .ortools_tmp/ortools.tar.gz -C .ortools --strip-components=1

rm -rf .ortools_tmp

echo "OR-Tools successfully installed to .ortools/"
