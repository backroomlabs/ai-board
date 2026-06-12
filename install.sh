#!/usr/bin/env sh
#
# AI Board installer: downloads the `abd` binary from GitHub Releases into
# ~/.local/bin and installs the skills into ~/.agents/skills (the cross-tool
# location read by Codex and Cursor; Claude Code users should install the
# plugin instead — see README).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/goblin-industries/ai-board/main/install.sh | sh
#   ABD_VERSION=v0.1.0 ... | sh    # pin a version (default: latest release)
#   NO_SKILLS=1 ... | sh           # binary only
set -eu

REPO="goblin-industries/ai-board"
VERSION="${ABD_VERSION:-latest}"
BIN_DIR="${ABD_BIN_DIR:-$HOME/.local/bin}"
SKILLS_DIR="${ABD_SKILLS_DIR:-$HOME/.agents/skills}"

if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/$REPO/releases/latest/download"
else
  BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
fi

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)
    case "$arch" in
      x86_64)        target="x86_64-unknown-linux-musl" ;;
      aarch64|arm64) target="aarch64-unknown-linux-musl" ;;
      *) echo "error: unsupported Linux arch: $arch" >&2; exit 1 ;;
    esac ;;
  Darwin)
    case "$arch" in
      x86_64)        target="x86_64-apple-darwin" ;;
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      *) echo "error: unsupported macOS arch: $arch" >&2; exit 1 ;;
    esac ;;
  *)
    echo "error: unsupported OS: $os" >&2
    echo "On Windows, download abd-x86_64-pc-windows-msvc.zip from:" >&2
    echo "  https://github.com/$REPO/releases" >&2
    exit 1 ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading abd ($target, $VERSION)..."
curl -fsSL "$BASE_URL/abd-$target.tar.gz" | tar -xz -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 755 "$tmp/abd" "$BIN_DIR/abd"
echo "Installed $BIN_DIR/abd"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "warning: $BIN_DIR is not on \$PATH — add it to your shell profile" >&2 ;;
esac

if [ "${NO_SKILLS:-0}" != "1" ]; then
  echo "Downloading skills..."
  curl -fsSL "$BASE_URL/skills.tar.gz" | tar -xz -C "$tmp"
  mkdir -p "$SKILLS_DIR"
  for skill in "$tmp"/skills/*/; do
    name="$(basename "$skill")"
    rm -rf "${SKILLS_DIR:?}/$name"
    cp -R "$skill" "$SKILLS_DIR/$name"
    echo "Installed skill $SKILLS_DIR/$name"
  done
  echo "Restart your agent (Codex/Cursor) to pick up the new skills."
  echo "Claude Code users: prefer the plugin instead —"
  echo "  /plugin marketplace add $REPO"
  echo "  /plugin install ai-board@ai-board"
fi

echo "Done. Try: abd --help"
