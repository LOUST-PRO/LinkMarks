//! `linkmarks completions <shell>` — emit a shell completion script.
//!
//! Operator-side convenience: instead of pinning a static file in
//! the repo (which goes stale the moment we add or rename a flag)
//! we regenerate from the live `Cli` parser at install time. The
//! script is printed to stdout so the caller can pipe it anywhere.
//!
//! Typical install flows:
//!
//! ```bash
//! # bash
//! linkmarks completions bash > ~/.local/share/bash-completion/completions/linkmarks
//!
//! # zsh (add to fpath, then `compinit`)
//! linkmarks completions zsh > "${fpath[1]}/_linkmarks"
//!
//! # fish
//! linkmarks completions fish > ~/.config/fish/completions/linkmarks.fish
//!
//! # powershell
//! linkmarks completions powershell > "$HOME\Documents\PowerShell\Completion\linkmarks.ps1"
//! ```

use anyhow::Result;
use clap::Args;
use clap_complete::Shell;
use std::io;

/// Arguments for `linkmarks completions`.
#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Which shell to emit the completion script for.
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Print a completion script for `args.shell` to stdout.
///
/// Returns `exit_codes::OK` on success. We do not need the resolved
/// `Paths` for completions — the script is regenerated from the live
/// `Cli` parser at install time, so there is no store or config IO.
pub fn run(args: CompletionsArgs, _paths: crate::Paths) -> Result<i32> {
    let mut cmd = crate::build_cli();
    let bin_name = cmd.get_name().to_string();
    // `clap_complete::generate` returns `()`. A write failure to a
    // piped target surfaces as a broken pipe on the next write; for
    // operator-side completion install that's acceptable (the shell
    // completion will simply be truncated and the user will see
    // an obvious install failure on the `source` step).
    clap_complete::generate(args.shell, &mut cmd, bin_name, &mut io::stdout().lock());
    Ok(crate::exit_codes::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    /// Parse the canonical CLI and run `clap_complete::generate` for
    /// the given shell into a `Vec<u8>`. We intentionally bypass the
    /// `run` entry point so the test does not touch stdout (which
    /// would race with `cargo test`'s own capture).
    fn emit(shell: Shell) -> Vec<u8> {
        let mut cmd = <crate::Cli as CommandFactory>::command();
        let bin = cmd.get_name().to_string();
        let mut buf: Vec<u8> = Vec::new();
        clap_complete::generate(shell, &mut cmd, bin, &mut buf);
        buf
    }

    #[test]
    fn bash_output_contains_subcommands_and_bin_name() {
        let script = String::from_utf8(emit(Shell::Bash)).expect("utf8");
        // bash completion uses `_comp_linkmarks` as the function
        // prefix derived from the bin name; the case statement
        // should mention our subcommands (we don't pin names — the
        // surface is owned by clap, this test is structural).
        assert!(script.contains("linkmarks"), "bin name missing");
        assert!(
            script.contains("init") || script.contains("list") || script.contains("tui"),
            "expected at least one subcommand literal"
        );
    }

    #[test]
    fn zsh_output_is_a_compdef_script() {
        let script = String::from_utf8(emit(Shell::Zsh)).expect("utf8");
        // zsh completions end with a `#compdef <bin>` footer so they
        // can be sourced via `compinit` from $fpath.
        assert!(
            script.contains("#compdef"),
            "zsh script missing #compdef footer"
        );
    }

    #[test]
    fn fish_output_uses_complete_command() {
        let script = String::from_utf8(emit(Shell::Fish)).expect("utf8");
        // Fish completions register via `complete -c <bin> ...`.
        assert!(
            script.contains("complete -c linkmarks"),
            "fish script missing `complete -c linkmarks` registration"
        );
    }

    #[test]
    fn powershell_output_uses_register_argumentcompleter() {
        let script = String::from_utf8(emit(Shell::PowerShell)).expect("utf8");
        // PowerShell completion is wired via Register-ArgumentCompleter.
        assert!(
            script.contains("Register-ArgumentCompleter"),
            "powershell script missing Register-ArgumentCompleter"
        );
    }

    #[test]
    fn elvish_output_uses_edit_completion_arg_completer() {
        let script = String::from_utf8(emit(Shell::Elvish)).expect("utf8");
        // Elvish completion uses `edit:completion:arg-completer`.
        assert!(
            script.contains("edit:completion:arg-completer"),
            "elvish script missing completion binding"
        );
    }

    #[test]
    fn all_supported_shells_appear_in_subcommand_help() {
        // Smoke: `--help` against the `completions` subcommand must
        // list every shell we expose. If clap renames or drops one
        // this test will surface it.
        let cli = crate::Cli::try_parse_from(["linkmarks", "completions", "--help"]);
        let help = format!("{:?}", cli).to_lowercase();
        assert!(help.contains("bash"));
        assert!(help.contains("zsh"));
        assert!(help.contains("fish"));
        assert!(help.contains("powershell"));
        assert!(help.contains("elvish"));
    }
}
