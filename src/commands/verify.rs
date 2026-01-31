use anyhow::Result;

use crate::cli::VerifyArgs;
use crate::config::Policy;
use crate::git::Git;
use crate::transcript::TranscriptStore;

pub(crate) fn cmd_verify(git: &Git, args: VerifyArgs, _verbose: bool) -> Result<u8> {
    let policy = Policy::load_from_repo(&git.repo)?;
    let store = TranscriptStore::git_notes();

    let commit = git.resolve_commitish(&args.commitish)?;
    let transcript = match store.load(&git.repo, &commit) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("aigit verify: {err}");
            return Ok(4);
        }
    };

    if let Some(t_commit) = &transcript.commit {
        if t_commit != &commit {
            eprintln!("aigit verify: transcript commit mismatch");
            return Ok(4);
        }
    }

    let expected_patch_id = git.patch_id_for_commit(&commit)?;
    if transcript.diff_fingerprint.patch_id != expected_patch_id {
        eprintln!("aigit verify: diff fingerprint mismatch");
        return Ok(4);
    }

    let ok = transcript.verify_against_policy(&policy);
    if ok {
        println!("aigit verify: PASS ({commit})");
        Ok(0)
    } else {
        println!("aigit verify: FAIL ({commit})");
        Ok(4)
    }
}

