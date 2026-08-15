# RPM packaging — downstream maintainer starter

This directory is a **declarative stub** for downstream packagers that
want to ship `linkmarks` as `.rpm` packages (Fedora / RHEL / openSUSE).
The upstream (LOUST-PRO/LinkMarks) does **not** build or sign packages.

## Layout

```text
rpm/
├── linkmarks.spec         # rpm-build spec (Name, Version, BuildRequires, …)
├── linkmarks.toml         # sample fedora-review config (downstream policy)
└── README.md              # this file
```

## Quick start (downstream)

```bash
# In a downstream `linkmarks-rpm-source` repo:
git clone https://github.com/LOUST-PRO/LinkMarks linkmarks-rpm-source
cd linkmarks-rpm-source
mkdir -p rpm
cp -rT /path/to/this/directory rpm/
rpmbuild -ba rpm/linkmarks.spec
```

The template builds with `cargo build --release` and installs
`/usr/bin/linkmarks` plus the manpage at
`/usr/share/man/man1/linkmarks.1.gz`. The `%check` section is a no-op
(upstream CI runs the test matrix; rebuilding it on every downstream
build is duplicate compute).

## Maintenance contract

- Bump `Version:` whenever upstream releases (no `Release:` rebuilds
  for in-place patches).
- Keep the `%license` macro aligned with `LICENSE` + `LICENSE-COMMERCIAL.md`.
- The `%install` section is the only place you can pull in distro-local
  policies (shell completions paths, SELinux labels, hardened-build flags).
- Divergence from the upstream anti-feature list requires a CONCERNS.md
  entry — these templates do not introduce any.

## Files

- [`linkmarks.spec`](./linkmarks.spec) — rpm-build spec.
