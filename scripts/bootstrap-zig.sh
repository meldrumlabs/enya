#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/.context/zig"
VERSION="0.15.2"

mkdir -p "${OUT_DIR}"

os="$(uname -s)"
arch="$(uname -m)"

case "${os}" in
  Darwin)
    case "${arch}" in
      x86_64)
        tarball="zig-x86_64-macos-${VERSION}.tar.xz"
        shasum="375b6909fc1495d16fc2c7db9538f707456bfc3373b14ee83fdd3e22b3d43f7f"
        ;;
      arm64)
        tarball="zig-aarch64-macos-${VERSION}.tar.xz"
        shasum="3cc2bab367e185cdfb27501c4b30b1b0653c28d9f73df8dc91488e66ece5fa6b"
        ;;
      *)
        echo "Unsupported arch: ${arch}" >&2
        exit 1
        ;;
    esac
    ;;
  Linux)
    case "${arch}" in
      x86_64)
        tarball="zig-x86_64-linux-${VERSION}.tar.xz"
        shasum="02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239"
        ;;
      aarch64)
        tarball="zig-aarch64-linux-${VERSION}.tar.xz"
        shasum="958ed7d1e00d0ea76590d27666efbf7a932281b3d7ba0c6b01b0ff26498f667f"
        ;;
      *)
        echo "Unsupported arch: ${arch}" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported OS: ${os}" >&2
    exit 1
    ;;
esac

url="https://ziglang.org/download/${VERSION}/${tarball}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

archive="${tmp_dir}/${tarball}"

echo "Downloading ${url}"
curl -fsSL -o "${archive}" "${url}"

echo "${shasum}  ${archive}" | shasum -a 256 -c -

dest="${OUT_DIR}/${VERSION}"
rm -rf "${dest}"
mkdir -p "${dest}"

tar -xf "${archive}" -C "${dest}" --strip-components=1

ln -sfn "${dest}/zig" "${OUT_DIR}/zig"

echo "Installed Zig ${VERSION} to ${OUT_DIR}/zig"
