# License

LinkMarks is dual-licensed:

**AGPL-3.0-or-later** (open source) **OR** **LicenseRef-Commercial**
(commercial).

You may choose which license applies to your use.

## AGPL-3.0-or-later

The open-source license is the GNU Affero General Public License,
version 3 or any later version published by the Free Software
Foundation. The full text is at [LICENSE](https://github.com/LOUST-PRO/LinkMarks/blob/main/LICENSE)
in the repo root.

The AGPL §13 network-use clause is the meaningful restriction:
if you run a modified version of LinkMarks as a network service,
you must publish the modified source under the same license.

## LicenseRef-Commercial

For entities that need to skip the AGPL §13 network-use clause
without publishing their modifications, a commercial license is
available. Contact `opensource@loust.pro` for terms.

The commercial license is what most enterprises use when they
want to embed LinkMarks into a SaaS product without the
AGPL disclosure obligation.

## Third-party dependencies

LinkMarks depends on the following crates. Each is used under
its own license:

| Crate | Version | License | Purpose |
|---|---|---|---|
| `rusqlite` | 0.32 | MIT | SQLite bindings |
| `clap` | 4.5 | MIT/Apache-2.0 | CLI parser |
| `ratatui` | 0.28 | MIT | TUI framework |
| `crossterm` | 0.28 | MIT | Terminal I/O |
| `nucleo` | 0.5 | MIT | Fuzzy matcher |
| `yrs` | 0.18 | MIT/Apache-2.0 | CRDT |
| `tokio` | 1.40 | MIT | Async runtime (sync layer) |
| `reqwest` | 0.12 | MIT/Apache-2.0 | HTTP client (sync layer) |
| `serde` | 1.0 | MIT/Apache-2.0 | Serialization |
| `serde_json` | 1.0 | MIT/Apache-2.0 | JSON |
| `toml` | 0.8 | MIT/Apache-2.0 | Config parsing |
| `ulid` | 1.1 | MIT | ULID generation |
| `url` | 2.5 | MIT/Apache-2.0 | URL parsing |
| `chrono` | 0.4 | MIT/Apache-2.0 | Timestamps |
| `anyhow` | 1.0 | MIT/Apache-2.0 | Error handling |
| `thiserror` | 1.0 | MIT/Apache-2.0 | Error types |
| `tracing` | 0.1 | MIT | Structured logging |
| `tracing-subscriber` | 0.3 | MIT | Log subscriber |
| `env_logger` | 0.11 | MIT/Apache-2.0 | Env-driven logger |

The full transitive list is in `Cargo.lock` and the SBOM at
`docs/sbom.json` in the repo root.

## SPDX expression

The Cargo.toml metadata uses the SPDX expression:

```text
License: AGPL-3.0-or-later OR LicenseRef-Commercial
```

The `OR` is intentional — the user picks one. See the SPDX
spec for the formal semantics.

## Contributing

Contributions are accepted under the same dual-license model.
By submitting a pull request, you agree to license your
contribution under both AGPL-3.0-or-later and
LicenseRef-Commercial.

See [CONTRIBUTING.md](https://github.com/LOUST-PRO/LinkMarks/blob/main/CONTRIBUTING.md)
for the contribution workflow.

## Trademark

"LinkMarks" is a project name. It is not a registered trademark.
You may use the name to refer to the unmodified version of this
project. You may not use the name to refer to a modified version
without explicit written permission.

## Contact

- General questions: GitHub Discussions
- Security disclosures: `security@loust.pro`
- Commercial licensing: `opensource@loust.pro`