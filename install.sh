#!/bin/sh
# Installer yakocode untuk macOS, Linux, dan Termux (Android).
#
#   curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/yakocode/main/install.sh | sh
#
# Env yang didukung:
#   YAKOCODE_VERSION     tag rilis, mis. v0.1.0 (default: latest)
#   YAKOCODE_INSTALL_DIR direktori install (default: $PREFIX/bin di Termux,
#                        selain itu $HOME/.local/bin)
set -eu

REPO="prototypeall850-creator/yakocode"
VERSION="${YAKOCODE_VERSION:-latest}"

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64)   echo "aarch64-apple-darwin" ;;
        x86_64)  echo "x86_64-apple-darwin" ;;
        *)       echo "arsitektur tidak didukung: $arch" >&2; exit 1 ;;
      esac
      ;;
    Linux)
      # Termux (Android)
      if [ "$(uname -o 2>/dev/null || echo unknown)" = "Android" ]; then
        case "$arch" in
          aarch64|arm64) echo "aarch64-linux-android" ;;
          *) echo "Termux di arsitektur $arch belum ada binary-nya, pakai cargo: cargo install --git https://github.com/$REPO" >&2; exit 1 ;;
        esac
        return
      fi
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-musl" ;;
        *)       echo "Linux $arch belum ada binary-nya, pakai cargo: cargo install --git https://github.com/$REPO" >&2; exit 1 ;;
      esac
      ;;
    *) echo "OS tidak didukung: $os" >&2; exit 1 ;;
  esac
}

default_install_dir() {
  # Termux: $PREFIX/bin sudah masuk PATH
  if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/bin" ]; then
    echo "$PREFIX/bin"
  else
    echo "$HOME/.local/bin"
  fi
}

TARGET="$(detect_target)"
INSTALL_DIR="${YAKOCODE_INSTALL_DIR:-$(default_install_dir)}"

if [ "$VERSION" = "latest" ]; then
  BASE="https://github.com/$REPO/releases/latest/download"
else
  BASE="https://github.com/$REPO/releases/download/$VERSION"
  TAG="$VERSION"
fi
TAG="${TAG:-$(basename "$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")")}"
ASSET="yakocode-$TAG-$TARGET.tar.gz"

echo "Install yakocode $TAG ($TARGET) ke $INSTALL_DIR ..."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM
curl -fsSL "$BASE/$ASSET" -o "$tmp/yakocode.tar.gz"
tar -xzf "$tmp/yakocode.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
cp "$tmp/yakocode-$TAG-$TARGET/yakocode" "$INSTALL_DIR/yakocode"
chmod +x "$INSTALL_DIR/yakocode"

echo "OK: $("$INSTALL_DIR/yakocode" --version)"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "NOTE: $INSTALL_DIR belum ada di PATH. Tambahkan ke shell rc:"; echo "  export PATH=\"\$PATH:$INSTALL_DIR\"" ;;
esac
