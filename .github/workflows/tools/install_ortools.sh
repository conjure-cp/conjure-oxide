#!/bin/bash
set -e

# Detect Ubuntu version
UBUNTU_VER=$(lsb_release -rs)
echo "Detected Ubuntu version: ${UBUNTU_VER}"

VERSION="9.11.4210"

if [ "${UBUNTU_VER}" = "24.04" ]; then
    URL="https://github.com/google/or-tools/releases/download/v${VERSION}/or-tools_amd64_ubuntu-24.04_cpp_v${VERSION}.tar.gz"
else
    # Default to 22.04 as it is standard and most widely compatible
    URL="https://github.com/google/or-tools/releases/download/v${VERSION}/or-tools_amd64_ubuntu-22.04_cpp_v${VERSION}.tar.gz"
fi

echo "Downloading OR-Tools from ${URL}..."
wget -q "${URL}" -O ortools.tar.gz

echo "Extracting..."
tar -xzf ortools.tar.gz

FOLDER=$(tar -tzf ortools.tar.gz | head -n 1 | cut -f1 -d"/")
echo "Root folder in tarball is: ${FOLDER}"

echo "Installing to /usr/local..."
sudo cp -r "${FOLDER}/include"/* /usr/local/include/
sudo cp -r "${FOLDER}/lib"/* /usr/local/lib/

# Register libraries
sudo ldconfig

echo "OR-Tools C++ library installed successfully!"
