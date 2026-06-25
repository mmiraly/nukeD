#!/usr/bin/env bash
set -euo pipefail

GITHUB_REPO="mmiraly/nukeD"
BINARY_NAME="nuked"
INSTALL_DIR="${NUKED_INSTALL_DIR:-/usr/local/bin}"

echo "nukeD installer"

OS="$(uname -s)"
case "$OS" in
    Linux*) OS_TYPE="linux" ;;
    Darwin*) OS_TYPE="darwin" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64) ARCH_TYPE="amd64" ;;
    aarch64|arm64) ARCH_TYPE="arm64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

LATEST_RELEASE="$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest")"
TAG_NAME="$(printf '%s' "$LATEST_RELEASE" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')"

if [ -z "$TAG_NAME" ]; then
    echo "Could not find latest release."
    exit 1
fi

ASSET_NAME="${BINARY_NAME}-${TAG_NAME}-${OS_TYPE}-${ARCH_TYPE}.tar.gz"
DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/${TAG_NAME}/${ASSET_NAME}"
TEMP_DIR="$(mktemp -d)"
TAR_FILE="${TEMP_DIR}/${ASSET_NAME}"

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "Downloading ${ASSET_NAME}"
curl -fL "$DOWNLOAD_URL" -o "$TAR_FILE"
tar -xzf "$TAR_FILE" -C "$TEMP_DIR"

if [ -w "$INSTALL_DIR" ]; then
    cp "$TEMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
else
    sudo cp "$TEMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
fi

echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}"
