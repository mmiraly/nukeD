#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-}"
TAP_DIR="${NUKED_TAP_DIR:-../Homebrew-Tap}"
FORMULA_FILE="${TAP_DIR}/Formula/nuked.rb"
REPO_URL="https://github.com/mmiraly/nukeD"
BINARY_NAME="nuked"

if [ -z "$TAG" ]; then
    echo "Usage: $0 <vX.Y.Z>"
    exit 1
fi

TEMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

download_and_sha() {
    local suffix="$1"
    local filename="${BINARY_NAME}-${TAG}-${suffix}.tar.gz"
    local url="${REPO_URL}/releases/download/${TAG}/${filename}"
    local output="${TEMP_DIR}/${filename}"

    curl -fL "$url" -o "$output" >/dev/null 2>&1
    shasum -a 256 "$output" | awk '{print $1}'
}

SHA_MAC_ARM="$(download_and_sha "darwin-arm64")"
SHA_MAC_INTEL="$(download_and_sha "darwin-amd64")"
SHA_LINUX_INTEL="$(download_and_sha "linux-amd64")"
SHA_LINUX_ARM="$(download_and_sha "linux-arm64")"

mkdir -p "$(dirname "$FORMULA_FILE")"
cat > "$FORMULA_FILE" <<EOF
class Nuked < Formula
  desc "Nuke stale project dependency folders."
  homepage "${REPO_URL}"
  version "${TAG}"
  license "GPL-3.0"

  on_macos do
    if Hardware::CPU.intel?
      url "${REPO_URL}/releases/download/${TAG}/${BINARY_NAME}-${TAG}-darwin-amd64.tar.gz"
      sha256 "${SHA_MAC_INTEL}"
    else
      url "${REPO_URL}/releases/download/${TAG}/${BINARY_NAME}-${TAG}-darwin-arm64.tar.gz"
      sha256 "${SHA_MAC_ARM}"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "${REPO_URL}/releases/download/${TAG}/${BINARY_NAME}-${TAG}-linux-amd64.tar.gz"
      sha256 "${SHA_LINUX_INTEL}"
    else
      url "${REPO_URL}/releases/download/${TAG}/${BINARY_NAME}-${TAG}-linux-arm64.tar.gz"
      sha256 "${SHA_LINUX_ARM}"
    end
  end

  def install
    bin.install "${BINARY_NAME}"
    zsh_completion.install "_nuked"
  end

  test do
    system "#{bin}/${BINARY_NAME}", "--help"
  end
end
EOF

(
    cd "$TAP_DIR"
    git add Formula/nuked.rb
    if git diff --staged --quiet; then
        echo "No tap changes."
    else
        git commit -m "add nuked formula"
        git push origin main
    fi
)
