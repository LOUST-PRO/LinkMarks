# Debian / Ubuntu packaging — downstream maintainer starter

This directory is a **declarative stub** for downstream packagers that want
to ship `linkmarks` as `.deb` packages. The upsteam (LOUST-PRO/LinkMarks)
does **not** build or sign packages; the templates here are starting points
that you can copy into a separate `linkmarks-debian` repo and adapt.

## Layout

```text
debian/
├── control      # package metadata (Source / Binary / Depends / Section)
├── rules        # debhelper rules (build → cargo build --release)
├── compat       # debhelper compatibility level
├── changelog    # one entry per upload
├── copyright    # DEP-5 / © header pointing at the LICENSE file
├── source/
│   └── format   # source format ("3.0 (quilt)")
└── README.md    # this file
```

## Quick start (downstream)

```bash
git clone https://github.com/LOUST-PRO/LinkMarks linkmarks-debian-source
cd linkmarks-debian-source
# Copy the templates next to the source tree:
mkdir -p debian
cp -rT /path/to/this/directory debian/
# Edit debian/changelog with your distro tag (`dch -v 2.1.0-1`).
dpkg-buildpackage -us -uc -b
```

The `debian/rules` template uses `cargo build --release` and installs
`target/release/linkmarks` as `/usr/bin/linkmarks`, plus the manpage at
`/usr/share/man/man1/linkmarks.1.gz`. It does **not** install shell
completions; downstream policy decides the right path
(`/usr/share/bash-completion/completions/` vs `vendor-ship` etc).

## Maintenance contract

- Bump `Standards-Version` when the upstream policy changes.
- Keep `debian/copyright` in sync with `LICENSE` + `LICENSE-COMMERCIAL.md`.
- Run `lintian -IE` after every build; treat `E` as a blocker, `W` as a
  good-citizen ask.
- Any divergence from the upstream anti-feature list (telemetry, AI,
  closed-source build) requires a CONCERNS.md entry — these templates
  do not introduce any.

## Files

- [`control`](./control) — Source + Binary metadata, Depends, Section.
- [`rules`](./rules) — `dh`/`dh_cargo` build pipeline.
- [`compat`](./compat) — debhelper `12`.
- [`changelog`](./changelog) — dist-tagged release log.
- [`copyright`](./copyright) — DEP-5 copyright header.
- [`source/format`](./source/format) — source format (3.0 quilt).
