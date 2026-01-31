use anyhow::Result;

use crate::cli::{HookMode, InstallHookArgs};
use crate::git::Git;

pub(crate) fn cmd_install_hook(git: &Git, args: InstallHookArgs) -> Result<u8> {
    match args.mode {
        HookMode::PreCommit => {
            git.install_pre_commit_hook(args.force)?;
            Ok(0)
        }
    }
}

