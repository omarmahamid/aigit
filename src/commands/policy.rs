use anyhow::Result;

use crate::config::Policy;
use crate::git::Git;

pub(crate) fn cmd_policy_validate(git: &Git, verbose: bool) -> Result<u8> {
    let policy = Policy::load_from_repo(&git.repo)?;
    if verbose {
        eprintln!("policy: {policy:#?}");
    }
    Ok(0)
}

