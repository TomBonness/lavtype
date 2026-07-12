#!/bin/sh
# Build one release artifact with the pinned sherpa-onnx native runtime.
# Usage: scripts/release.sh macos arm64|x86_64 | linux x86_64
set -eu

platform=${1:-}
arch=${2:-}
if [ -z "$platform" ] || [ -z "$arch" ]; then
  echo "usage: $0 macos arm64|x86_64 | linux x86_64" >&2
  exit 2
fi

version=1.13.4
base_url="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${version}"
case "$platform/$arch" in
  linux/x86_64)
    target=x86_64-unknown-linux-gnu
    archive=sherpa-onnx-v1.13.4-linux-x64-static-lib.tar.bz2
    digest=98b0e31996426f6e78244dbce1955548f2c64e8f01c4be75b85af7cdaa2e8d5c
    format=appimage
    ;;
  macos/arm64)
    target=aarch64-apple-darwin
    archive=sherpa-onnx-v1.13.4-osx-arm64-static-lib.tar.bz2
    digest=57801db2bbb786a5d343f515a38ff210b401842338bdc804fa075312d1cd2404
    format=dmg
    ;;
  macos/x86_64)
    target=x86_64-apple-darwin
    archive=sherpa-onnx-v1.13.4-osx-x64-static-lib.tar.bz2
    digest=2bda2c10b31a1cfc45d9f9e14bd4983743ec3779d309e42d99a6c8fa1689043f
    format=dmg
    ;;
  *)
    echo "unsupported platform/architecture: $platform/$arch" >&2
    exit 2
    ;;
esac

mkdir -p .cache/sherpa-onnx dist
archive_path=.cache/sherpa-onnx/$archive
if [ ! -f "$archive_path" ]; then
  curl --fail --location --retry 3 --output "$archive_path" "$base_url/$archive"
fi
if command -v sha256sum >/dev/null 2>&1; then
  printf '%s  %s\n' "$digest" "$archive_path" | sha256sum -c -
else
  printf '%s  %s\n' "$digest" "$archive_path" | shasum -a 256 -c -
fi
export SHERPA_ONNX_ARCHIVE_DIR=$(CDPATH= cd -- "$(dirname "$archive_path")" && pwd)

rustup target add "$target"
cargo build --release --locked --target "$target"
cargo install cargo-packager --version 0.11.8 --locked
cp "target/$target/release/lavtype" dist/lavtype
cargo packager --release --target "$target" --formats "$format"
rm -f dist/lavtype

# Leave a deterministic checksum file beside every generated package.
: > dist/SHA256SUMS
for artifact in dist/*.dmg dist/*.AppImage; do
  if [ -f "$artifact" ]; then
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$artifact" >> dist/SHA256SUMS; else shasum -a 256 "$artifact" >> dist/SHA256SUMS; fi
  fi
done
