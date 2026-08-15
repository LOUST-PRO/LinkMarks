# SPDX-License-Identifier: AGPL-3.0-or-later
#
# linkmarks.spec — rpm-build spec template for downstream maintainers.
#
# Maintainer: LOUST-PRO Packaging <opensource@loust.pro>
# Upstream: https://github.com/LOUST-PRO/LinkMarks
#
# This file is a STARTER. Adapt Version:, Release:, Source0:, %changelog,
# and BuildRequires: to your distro conventions before running rpmbuild.

Name:           linkmarks
Version:        2.1.0
Release:        1%{?dist}
Summary:        Local-first bookmark manager
License:        AGPL-3.0-or-later AND LicenseRef-Commercial
URL:            https://github.com/LOUST-PRO/LinkMarks
Source0:        https://github.com/LOUST-PRO/LinkMarks/archive/refs/tags/v%{version}.tar.gz
BuildArch:      %{_target_cpu}
BuildRequires:  cargo >= 1.78
BuildRequires:  rustc >= 1.78
BuildRequires:  gzip
BuildRequires:  systemd-rpm-macros

%description
LinkMarks imports bookmarks from Chromium, Firefox, and Netscape HTML,
dedupes by canonical URL with a human-readable conflict report, and
exposes a ratatui-based TUI. Storage is a local SQLite store (XDG
paths) with WAL + busy_timeout for safe concurrent access.

Dual-licensed: AGPL-3.0-or-later (network clause applies) plus a
Commercial license for entities that need to skip §13. See
LICENSE-COMMERCIAL.md in the source tree.

%prep
%autosetup -n LinkMarks-%{version}

%build
cargo build --release --bin linkmarks

%install
install -Dpm 0755 target/release/linkmarks %{buildroot}%{_bindir}/linkmarks

# Manpage (built upstream; downstream can swap for pandoc)
if [ -f docs/man/linkmarks.1 ]; then
  install -Dpm 0644 docs/man/linkmarks.1 \
    %{buildroot}%{_mandir}/man1/linkmarks.1
  gzip -9n %{buildroot}%{_mandir}/man1/linkmarks.1
fi

# License files
install -Dpm 0644 LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE
install -Dpm 0644 LICENSE-COMMERCIAL.md \
  %{buildroot}%{_licensedir}/%{name}/LICENSE-COMMERCIAL.md

%files
%license %{_licensedir}/%{name}/LICENSE
%license %{_licensedir}/%{name}/LICENSE-COMMERCIAL.md
%{_bindir}/linkmarks
%{_mandir}/man1/linkmarks.1.gz

%changelog
* Fri Aug 15 2026 LOUST-PRO Packaging <opensource@loust.pro> - 2.1.0-1
- Upstream release 2.1.0 (F2.5 batch: fuzzy filter, sort modes,
  shell completions for bash/zsh/fish/powershell/elvish).
