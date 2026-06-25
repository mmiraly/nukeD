#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-}"
BINARY_NAME="nuked"

if [ -z "$TAG" ]; then
    echo "Usage: $0 <vX.Y.Z>"
    exit 1
fi

if ! command -v gh >/dev/null 2>&1; then
    echo "gh CLI is required."
    exit 1
fi

TEMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

build_and_package() {
    local target_triple="$1"
    local arch_name="$2"
    local tar_name="${BINARY_NAME}-${TAG}-darwin-${arch_name}.tar.gz"
    local build_dir="${TEMP_DIR}/darwin-${arch_name}"

    echo "Building ${target_triple}"
    cargo build --release --target "$target_triple" --quiet

    mkdir -p "$build_dir"
    cp "target/${target_triple}/release/${BINARY_NAME}" "$build_dir/"
    cp "completions/zsh/_nuked" "$build_dir/"

    tar -czf "$tar_name" -C "$build_dir" .
    shasum -a 256 "$tar_name"
}

build_and_package "aarch64-apple-darwin" "arm64"
build_and_package "x86_64-apple-darwin" "amd64"

if ! gh release view "$TAG" >/dev/null 2>&1; then
    gh release create "$TAG" --title "$TAG" --generate-notes
fi

gh release upload "$TAG" "${BINARY_NAME}-${TAG}-darwin-arm64.tar.gz" --clobber
gh release upload "$TAG" "${BINARY_NAME}-${TAG}-darwin-amd64.tar.gz" --clobber

rm -f "${BINARY_NAME}-${TAG}-darwin-arm64.tar.gz"
rm -f "${BINARY_NAME}-${TAG}-darwin-amd64.tar.gz"

echo "Uploaded macOS release assets for ${TAG}"
