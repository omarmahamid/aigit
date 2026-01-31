#!/usr/bin/env sh
set -eu

REPO_DEFAULT="omarmahamid/aigit"
REPO="${AIGIT_REPO:-$REPO_DEFAULT}"
VERSION="${AIGIT_VERSION:-latest}"
INSTALL_DIR="${AIGIT_INSTALL_DIR:-$HOME/.local/bin}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  OS_TAG="unknown-linux-gnu" ;;
  Darwin) OS_TAG="apple-darwin" ;;
  *) echo "unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH_TAG="x86_64" ;;
  arm64|aarch64) ARCH_TAG="aarch64" ;;
  *) echo "unsupported arch: $ARCH" >&2; exit 1 ;;
esac

TARGET="${ARCH_TAG}-${OS_TAG}"
ASSET="aigit-${TARGET}.tar.gz"

API_URL="https://api.github.com/repos/${REPO}/releases/${VERSION}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$INSTALL_DIR"

echo "aigit: installing from ${REPO} (${VERSION}) for ${TARGET}"

JSON="$TMP_DIR/release.json"
curl -fsSL "$API_URL" -o "$JSON"

URL="$(python3 - <<PY
import json
import sys
data=json.load(open("$JSON","r"))
assets=data.get("assets",[])
name="${ASSET}"
for a in assets:
    if a.get("name")==name:
        print(a.get("browser_download_url",""))
        sys.exit(0)
print("")
sys.exit(0)
PY
)"

if [ -z "$URL" ]; then
  echo "aigit: could not find release asset ${ASSET} in ${API_URL}" >&2
  exit 1
fi

TAR="$TMP_DIR/$ASSET"
curl -fsSL "$URL" -o "$TAR"

tar -xzf "$TAR" -C "$TMP_DIR"
chmod +x "$TMP_DIR/aigit"
mv "$TMP_DIR/aigit" "$INSTALL_DIR/aigit"

echo "aigit: installed to $INSTALL_DIR/aigit"
echo "aigit: ensure $INSTALL_DIR is on your PATH"

