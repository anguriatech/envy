//! Envy library root — re-exports all four architecture layers.
//!
//! The binary entry point (`main.rs`) calls [`cli::run`], which owns the full
//! application lifecycle. Integration tests in `tests/` import this crate to
//! exercise the CLI, core, crypto, and database layers together without going
//! through the binary.
pub mod cli;
pub mod core;
pub mod crypto;
pub mod db;
