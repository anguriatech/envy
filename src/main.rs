//! Binary entry point for the `envy` CLI.
//!
//! Delegates entirely to [`envy::cli::run`], which parses arguments, manages
//! the vault lifecycle, and returns a POSIX exit code. The sole responsibility
//! of `main` is to forward that code to the OS via [`std::process::exit`].

use std::process;

fn main() {
    process::exit(envy::cli::run());
}
