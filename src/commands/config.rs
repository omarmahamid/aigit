use std::path::PathBuf;

use anyhow::Result;

use crate::cli::ConfigSetArgs;
use crate::config::Policy;
use crate::git::Git;

pub(crate) fn cmd_config_set(git: &Git, args: ConfigSetArgs) -> Result<u8> {
    let mut policy = Policy::load_from_repo(&git.repo)?;
    policy.set_key(&args.key, &args.value)?;
    let path: PathBuf = git.repo.workdir.join(".aigit.toml");
    std::fs::write(&path, policy.to_toml_string()?)?;
    println!("wrote {}", path.display());
    Ok(0)
}

