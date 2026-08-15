# Homebrew packaging — downstream maintainer starter

This directory is a **declarative stub** for downstream packagers that
want to ship `linkmarks` as a [Homebrew](https://brew.sh) tap
(`brew install louzt/tap/linkmarks` or
`brew install --HEAD louzt/tap/linkmarks`).
The upstream (LOUST-PRO/LinkMarks) does **not** maintain the tap.

## Layout

```text
homebrew/
├── linkmarks.rb              # formula
└── README.md                 # this file
```

## Quick start (downstream)

```bash
# Create the tap:
brew tap-new louzt/tap
# Drop the formula in:
cp /path/to/this/directory/linkmarks.rb \
   $(brew --repository louzt/tap)/Formula/linkmarks.rb
# Audit and install:
brew audit --new --online louzt/tap/linkmarks
brew install louzt/tap/linkmarks
```

The template installs `linkmarks` to `$HOMEBREW_PREFIX/bin` and ships the
manpage under `share/man/1`. Shell completions are NOT installed by
Homebrew — operators who want them pip the `linkmarks completions <shell>`
output into their dotfiles.

## Maintenance contract

- Bump `version` whenever upstream releases.
- `livecheck` is the canonical block for upstream-driven updates;
  test it with `brew livecheck --debug louzt/tap/linkmarks`.
- Run `brew audit --strict --online louzt/tap/linkmarks` after every
  push; treat any audit hint as a blocker.
- Use `brew install --build-from-source louzt/tap/linkmarks` to verify
  the `system "cargo", "build", ...` line locally before pushing.
- Divergence from the upstream anti-feature list requires a CONCERNS.md
  entry — this template introduces none.

## Files

- [`linkmarks.rb`](./linkmarks.rb) — the Homebrew formula.
