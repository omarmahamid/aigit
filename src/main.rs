mod app;
mod cli;
mod config;
mod codex_cli;
mod commands;
mod examiner;
mod git;
mod redact;
mod transcript;

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(app::run())
}
