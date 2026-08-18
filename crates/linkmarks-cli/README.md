# linkmarks-cli

Command-line interface for LinkMarks. Drives the `linkmarks` binary
(dispatched by the umbrella crate): `init`, `list`, `import`, `export`,
`dedupe`, `sync`, `completions`, plus the help/version wrappers.

This crate exposes the `linkmarks_cli::run() -> Result<i32>` entry point
that the umbrella `linkmarks` binary calls. It is also a library so
embedders can wire the same subcommands into their own binary.

## What is it

`linkmarks-cli` is the deterministic surface of the workspace. Every
subcommand:

- Accepts `--format=table|json|yaml` and emits structured output.
- Returns stable exit codes (see `linkmarks_cli::exit_codes`).
- Supports `--store <path>` and `--config <path>` overrides that fall
  back to `LINKMARKS_STORE` / `LINKMARKS_CONFIG` env, then XDG defaults.
- Emits `init` / `import` / `export` reports as key=value pairs that
  are easy to grep and pipe.

The `completions` subcommand regenerates shell completion scripts for
bash, zsh, fish, PowerShell, and Elvish from the live `cli` parser each
invocation; a renamed flag surfaces immediately.

## Install

You almost always want the umbrella binary:

```bash
cargo install linkmarks --locked
```

To depend on the CLI logic as a library instead:

```toml
[dependencies]
linkmarks-cli = "2.2.0"
```

Then call `linkmarks_cli::run()` from your own `main()`.

## Usage

```rust
fn main() {
    match linkmarks_cli::run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}
```

The `build_cli()` function returns the configured `clap::Command` so
callers can embed LinkMarks as a subcommand of a larger tool.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
