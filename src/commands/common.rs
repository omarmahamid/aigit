use anyhow::Result;

use crate::config::Policy;
use crate::examiner::{CodexCliExaminer, Examiner, StaticExaminer};
use crate::git::Git;

pub(crate) fn load_policy_verbose(git: &Git, verbose: bool) -> Result<Policy> {
    let policy = Policy::load_from_repo(&git.repo)?;
    if verbose {
        let policy_path = git.repo.workdir.join(".aigit.toml");
        eprintln!(
            "aigit: policy file: {} ({})",
            policy_path.display(),
            if policy_path.exists() {
                "present"
            } else {
                "missing (using defaults)"
            }
        );
        eprintln!(
            "aigit: provider: {}",
            policy.provider.clone().unwrap_or_else(|| "local".to_string())
        );
    }
    Ok(policy)
}

pub(crate) fn examiner_label(policy: &Policy) -> &'static str {
    match policy.provider.as_deref() {
        Some("codex-cli") => "codex-cli",
        _ => "local-static",
    }
}

pub(crate) fn build_examiner(policy: &Policy) -> Box<dyn Examiner> {
    match policy.provider.as_deref() {
        Some("codex-cli") => Box::new(CodexCliExaminer::new(policy)),
        _ => Box::new(StaticExaminer::new()),
    }
}

