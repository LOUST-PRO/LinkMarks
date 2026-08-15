# linkmarks.rb — Homebrew formula template for downstream maintainers.
#
# Upstream:  https://github.com/LOUST-PRO/LinkMarks
# License:   AGPL-3.0-or-later AND Commercial (see LICENSE-COMMERCIAL.md)
#
# This is a STARTER. Bump `version` (auto-extracted from the URL tag)
# and `sha256` against the upstream v2.x tag. Ship this in
# `louzt/tap/Formula/linkmarks.rb`.

class Linkmarks < Formula
  desc "Local-first bookmark manager (CLI + TUI + bridges)"
  homepage "https://github.com/LOUST-PRO/LinkMarks"
  url "https://github.com/LOUST-PRO/LinkMarks/archive/refs/tags/v2.1.0.tar.gz"
  sha256 "REPLACE_WITH_UPSTREAM_TARBALL_SHA256"

  license any_of: [
    "AGPL-3.0-or-later",
    "Commercial",
  ]

  depends_on "rust" => :build

  # The Cargo workspace has 6 crates; the canonical build target is the
  # `linkmarks` binary in `crates/linkmarks-cli`. `--locked` pins to
  # Cargo.lock so downstream cargo feature resolution matches CI.
  def install
    system "cargo", "install",
           "--path", "crates/linkmarks-cli",
           "--locked",
           "--root", prefix

    man1.install "docs/man/linkmarks.1" if File.exist?("docs/man/linkmarks.1")
  end

  test do
    assert_match "linkmarks", shell_output("#{bin}/linkmarks --version")
    output = shell_output("#{bin}/linkmarks --help")
    %w[init list import export dedupe tui completions].each do |cmd|
      assert_match cmd, output
    end
  end

  # livecheck is the canonical mechanism for upstream-driven updates
  # in the tap repo. The block below tracks v-tagged releases.
  livecheck do
    url :stable
    strategy :github_latest
  end
end
