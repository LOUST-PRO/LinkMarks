## What

<!-- One-line summary of the change. -->

## Why

<!-- Reference the SPEC, ROADMAP, ADR, or issue this PR advances.
     Cite the file:line where relevant. Do NOT open a PR for cosmetics
     or convention. -->

## How

<!-- Implementation notes: new crates, new traits, new dependencies. -->

## Validation

<!-- Paste actual command output. No claims of "works on my machine". -->

```
cargo build --release
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Scope boundary

<!-- What this PR does NOT include. Anti-features that look like they
     could be added here but won't be. -->

## Risk asymmetry audit (if outbound)

<!-- For PRs that touch external APIs: reward vs loss, recovery time.
     Default DECLINE if reward=cosmetic and loss=irreversible. -->

## Checklist

- [ ] `cargo fmt --check` passes locally.
- [ ] `cargo clippy --all-targets -- -D warnings` passes locally.
- [ ] `cargo test --all` passes locally.
- [ ] No new public types without an ADR or CONCERNS.md entry.
- [ ] No new network calls (verify with `strace` if unsure).
- [ ] No telemetry, no update-notifier.
- [ ] No absolute paths (`/home/lou`, `/root`, internal IPs) in diff.
- [ ] PII sanitization applied to any new fixtures.
