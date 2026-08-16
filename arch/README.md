# Arch Linux packaging — downstream maintainer starter

This directory is a **declarative stub** for downstream packagers that
want to ship `linkmarks` in the AUR (or a custom Arch channel).
The upstream (LOUST-PRO/LinkMarks) does **not** maintain the AUR
package.

## Layout

```text
arch/
├── PKGBUILD             # makepkg recipe (downstream policy + checksums)
├── linkmarks.install    # pacman install hooks (XDG dirs, compdef)
├── .SRCINFO             # generated — DO NOT edit by hand
└── README.md            # this file
```

## Quick start (downstream)

```bash
# In a downstream AUR-style repo:
git clone https://github.com/LOUST-PRO/LinkMarks linkmarks-arch-source
cd linkmarks-arch-source
mkdir -p arch
cp -rT /path/to/this/directory arch/
cd arch
# Bump `pkgver=` and `sha256sums=` against the upstream v2.1.0 release.
makepkg -si
```

The template installs `/usr/bin/linkmarks` and `man1/linkmarks.1.gz`. The
`.install` script is a placeholder for XDG-dir bootstrap; downstream
maintainers usually delete this if they consider it cargo-culted.

## Maintenance contract

- Bump `pkgver=` whenever upstream releases. `sha256sums=` is mandatory;
  pass `--skipchecksums` only for ad-hoc local builds, never for upload.
- Run `namcap *.pkg.tar.*` after `makepkg`; treat `E` (error) as a
  blocker, `W` (warning) as a good-citizen ask.
- Divergence from the upstream anti-feature list requires a CONCERNS.md
  entry — these templates do not introduce any.

## Files

- [`PKGBUILD`](./PKGBUILD) — makepkg recipe.
- [`linkmarks.install`](./linkmarks.install) — pacman install hooks
  (XDG-dir idempotent create).
- [`.SRCINFO`](./.SRCINFO) — regenerated via `makepkg --allsource` or
  `makepkg --printsrcinfo`.
