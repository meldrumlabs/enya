#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/.context/zig"
VERSION="0.14.1"

mkdir -p "${OUT_DIR}"

os="$(uname -s)"
arch="$(uname -m)"

case "${os}" in
  Darwin)
    case "${arch}" in
      x86_64)
        tarball="zig-x86_64-macos-${VERSION}.tar.xz"
        shasum="b0f8bdfb9035783db58dd6c19d7dea89892acc3814421853e5752fe4573e5f43"
        ;;
      arm64)
        tarball="zig-aarch64-macos-${VERSION}.tar.xz"
        shasum="39f3dc5e79c22088ce878edc821dedb4ca5a1cd9f5ef915e9b3cc3053e8faefa"
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
        shasum="28a8b8a69a25e3e5f5e8f0c44cf80ebb49c84e3c7d2a7acb68de881268d93cb8"
        ;;
      aarch64)
        tarball="zig-aarch64-linux-${VERSION}.tar.xz"
        shasum="7a8b0ec6f0d9e0c92d7d2f7a22bc0de3e6a3c0a0d3c0b4c0c5c0d3e0f0a0b0c0"
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
