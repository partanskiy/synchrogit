//! synchrogit library crate. The binary at `src/main.rs` is a thin wrapper
//! around the modules exposed here. Integration tests in `tests/` consume the
//! crate through this entry point.

pub mod cli;
pub mod clock;
pub mod config;
pub mod error;
pub mod git;
pub mod ipc;
pub mod log_setup;
pub mod runtime;
pub mod state;
pub mod util;
pub mod worker;

pub use error::{Result, SynchrogitError};
