//! `linkmarks` umbrella binary — single `cargo install linkmarks` produces
//! the CLI binary.
//!
//! All real logic lives in the `linkmarks-cli` crate (which is dual
//! lib+bin). This file is intentionally minimal: it just delegates to
//! `linkmarks_cli::run()` and translates `Result<i32>` into a process
//! exit code.

fn main() {
    match linkmarks_cli::run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}
