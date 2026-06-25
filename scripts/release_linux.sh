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

sha256_file() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1"
    else
        sha256sum "$1"
    fi
}

build_and_package() {
    local target_triple="$1"
    local arch_name="$2"
    local linker="${target_triple}-gcc"
    local linker_var
    local tar_name="${BINARY_NAME}-${TAG}-linux-${arch_name}.tar.gz"
    local build_dir="${TEMP_DIR}/linux-${arch_name}"

    if ! command -v "$linker" >/dev/null 2>&1; then
        echo "Missing linker: ${linker}"
        echo "Install with: brew install messense/macos-cross-toolchains/${target_triple}"
        return 1
    fi

    linker_var="CARGO_TARGET_$(printf '%s' "$target_triple" | tr '[:lower:]-' '[:upper:]_')_LINKER"
    export "$linker_var=$linker"

    echo "Building ${target_triple}"
    cargo build --release --target "$target_triple" --quiet

    mkdir -p "$build_dir"
    cp "target/${target_triple}/release/${BINARY_NAME}" "$build_dir/"
    cp "completions/zsh/_nuked" "$build_dir/"

    tar -czf "$tar_name" -C "$build_dir" .
    sha256_file "$tar_name"
}

build_and_package "x86_64-unknown-linux-gnu" "amd64"
build_and_package "aarch64-unknown-linux-gnu" "arm64"

if ! gh release view "$TAG" >/dev/null 2>&1; then
    gh release create "$TAG" --title "$TAG" --generate-notes
fi

for asset in "${BINARY_NAME}-${TAG}-linux-"*.tar.gz; do
    [ -f "$asset" ] || continue
    gh release upload "$TAG" "$asset" --clobber
    rm -f "$asset"
done

echo "Uploaded Linux release assets for ${TAG}"
