//! Cargo frontend for the complete rlib analysis suite.

mod commands;
mod cli;

use std::env;
use std::io::{self, IsTerminal as _};
use std::process::ExitCode;

use color_eyre::config::{HookBuilder, Theme};

/// Runs the CLI dispatcher and converts its stable status to a process exit code.
fn main() -> ExitCode {
    let has_color_output = io::stderr().is_terminal() && env::var_os("NO_COLOR").is_none();
    let theme = if has_color_output {
        Theme::dark()
    } else {
        Theme::new()
    };

    // Reports must not be constructed before color-eyre owns the global hook.
    if let Err(error) = HookBuilder::new()
        .theme(theme)
        .display_env_section(false)
        .install()
    {
        eprintln!("cargo rlib: could not install the error reporter: {error}");
        return ExitCode::from(2);
    }
    ExitCode::from(cli::run())
}
