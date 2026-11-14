#!/bin/sh
# Prints the Homebrew formula for a release. Usage: homebrew-formula.sh <version> <dir with the .sha256 files>
set -eu
version=$1
dir=$2
sha() { cut -d' ' -f1 "$dir/keyloom-$1.tar.gz.sha256"; }
url() { echo "https://github.com/LucaKaufmann/keyloom/releases/download/v$version/keyloom-$1.tar.gz"; }
cat <<EOF
class Keyloom < Formula
  desc "CLI and MCP server for AI agents to see, audit and improve ZSA keyboard layouts"
  homepage "https://github.com/LucaKaufmann/keyloom"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$(url aarch64-apple-darwin)"
      sha256 "$(sha aarch64-apple-darwin)"
    end
    on_intel do
      url "$(url x86_64-apple-darwin)"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    on_arm do
      url "$(url aarch64-unknown-linux-gnu)"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    end
    on_intel do
      url "$(url x86_64-unknown-linux-gnu)"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "keyloom"
    pkgshare.install "SKILL.md"
  end

  def caveats
    <<~EOS
      To teach Claude Code the keyloom CLI, install the skill:
        mkdir -p ~/.claude/skills/keyloom && cp #{opt_pkgshare}/SKILL.md ~/.claude/skills/keyloom/
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/keyloom --version")
  end
end
EOF
