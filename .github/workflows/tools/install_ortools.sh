# Installation script because it doesn't exist an official package of OR-Tools for Ubuntu
# and Or-tools depends on abseil (google framework) and protobuf.

set -e
set -x

UBUNTU_VER=$(lsb_release -rs)
echo "Detected Ubuntu version: ${UBUNTU_VER}"

TAG="v9.11"
VERSION="9.11.4210"

if [ "${UBUNTU_VER}" = "24.04" ]; then
    URL="https://github.com/google/or-tools/releases/download/${TAG}/or-tools_amd64_ubuntu-24.04_cpp_v${VERSION}.tar.gz"
else
    URL="https://github.com/google/or-tools/releases/download/${TAG}/or-tools_amd64_ubuntu-24.04_cpp_v${VERSION}.tar.gz"
fi

echo "Downloading OR-Tools from ${URL}..."
wget -q "${URL}" -O ortools.tar.gz

echo "Extracting..."
mkdir -p ortools_extracted
tar -xzf ortools.tar.gz -C ortools_extracted --strip-components=1

echo "Installing to /usr/local..."
sudo cp -r ortools_extracted/include/* /usr/local/include/
sudo cp -r ortools_extracted/lib/* /usr/local/lib/

# Register libraries
sudo ldconfig

if [ -f "/usr/local/include/ortools/base/base_export.h" ]; then
    echo "OR-Tools C++ library installed successfully!"
else
    echo "ERROR: base_export.h not found in /usr/local/include/ortools/base/"
    ls -R /usr/local/include
    exit 1
fi
