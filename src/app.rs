use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands, ConfigCmd, PolicyCmd};
use crate::git::{Git, GitRepo};

pub(crate) fn run() -> u8 {
    match try_run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("aigit: {err}");
            1
        }
    }
}

fn try_run() -> Result<u8> {
    let cli = Cli::parse();

    let repo = match GitRepo::discover() {
        Ok(r) => r,
        Err(_) => {
            eprintln!("aigit: not a git repository");
            return Ok(1);
        }
    };
    let git = Git::new(repo);

    match cli.command {
        Commands::Exam(args) => crate::commands::exam::cmd_exam(&git, args, cli.verbose),
        Commands::Commit(args) => crate::commands::commit::cmd_commit(&git, args, cli.verbose),
        Commands::Verify(args) => crate::commands::verify::cmd_verify(&git, args, cli.verbose),
        Commands::InstallHook(args) => crate::commands::install_hook::cmd_install_hook(&git, args),
        Commands::Policy { command } => match command {
            PolicyCmd::Validate => crate::commands::policy::cmd_policy_validate(&git, cli.verbose),
        },
        Commands::Config { command } => match command {
            ConfigCmd::Set(args) => crate::commands::config::cmd_config_set(&git, args),
        },
    }
}
