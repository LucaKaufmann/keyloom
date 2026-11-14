#!/bin/sh
# Installs the latest keyloom release binary.
#   curl -fsSL https://github.com/LucaKaufmann/keyloom/releases/latest/download/install.sh | sh
# Options (environment): KEYLOOM_INSTALL_DIR (default ~/.local/bin), KEYLOOM_VERSION (default latest, e.g. 0.1.0),
# KEYLOOM_SKILL=1 also installs the Claude Code skill to ~/.claude/skills/keyloom.
set -eu

repo="LucaKaufmann/keyloom"
dir="${KEYLOOM_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *) echo "keyloom: no prebuilt binary for $(uname -s); build from source: https://github.com/$repo" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  arm64 | aarch64) arch="aarch64" ;;
  x86_64 | amd64) arch="x86_64" ;;
  *) echo "keyloom: no prebuilt binary for $(uname -m); build from source: https://github.com/$repo" >&2; exit 1 ;;
esac

name="keyloom-$arch-$os"
if [ -n "${KEYLOOM_VERSION:-}" ]; then
  base="https://github.com/$repo/releases/download/v$KEYLOOM_VERSION"
else
  base="https://github.com/$repo/releases/latest/download"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $name…"
curl -fsSL "$base/$name.tar.gz" -o "$tmp/$name.tar.gz"
curl -fsSL "$base/$name.tar.gz.sha256" -o "$tmp/$name.tar.gz.sha256"

want=$(cut -d' ' -f1 "$tmp/$name.tar.gz.sha256")
if command -v shasum >/dev/null 2>&1; then
  got=$(shasum -a 256 "$tmp/$name.tar.gz" | cut -d' ' -f1)
else
  got=$(sha256sum "$tmp/$name.tar.gz" | cut -d' ' -f1)
fi
if [ "$want" != "$got" ]; then
  echo "keyloom: checksum mismatch, not installing" >&2
  exit 1
fi

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$dir"
install -m 755 "$tmp/$name/keyloom" "$dir/keyloom"
echo "Installed $("$dir/keyloom" --version) to $dir/keyloom"

if [ "${KEYLOOM_SKILL:-}" = "1" ]; then
  mkdir -p "$HOME/.claude/skills/keyloom"
  cp "$tmp/$name/SKILL.md" "$HOME/.claude/skills/keyloom/SKILL.md"
  echo "Installed the Claude Code skill to ~/.claude/skills/keyloom"
fi

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Add $dir to your PATH to run keyloom." ;;
esac
