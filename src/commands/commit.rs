use anyhow::{anyhow, Context, Result};

use crate::cli::CommitArgs;
use crate::examiner::{ExamContext, Examiner};
use crate::git::Git;
use crate::transcript::{Decision, TranscriptStore};

use super::common;

pub(crate) fn cmd_commit(git: &Git, args: CommitArgs, verbose: bool) -> Result<u8> {
    let policy = common::load_policy_verbose(git, verbose)?;

    let (diff, changed_files) = git.diff_staged()?;
    if diff.trim().is_empty() {
        return Err(anyhow!("no staged changes to commit"));
    }

    let diff_patch_id = git.patch_id_from_diff_text(&diff)?;
    let (redacted_diff, redactions) = crate::redact::redact_diff(&policy, &diff)?;
    let ctx = ExamContext::new(
        git,
        diff_patch_id,
        &redacted_diff,
        changed_files,
        redactions,
        &policy,
    )?;

    let examiner: Box<dyn Examiner> = common::build_examiner(&policy);
    if verbose {
        eprintln!("aigit: examiner: {}", common::examiner_label(&policy));
    }
    let exam = examiner.generate_exam(&ctx)?;
    let answers = crate::transcript::Answers::prompt_tui(&exam)?;
    let score = examiner.grade_exam(&ctx, &exam, &answers)?;
    let decision = crate::transcript::Decision::from_score(&policy, &exam, &answers, &score);

    let mut transcript =
        crate::transcript::Transcript::from_exam_result(git, &policy, &ctx, &exam, &answers, &score, decision)?;

    if verbose {
        eprintln!("exam decision: {:?}", transcript.decision);
    }
    crate::transcript::print_human_result(&transcript);
    if transcript.decision != Decision::Pass {
        return Ok(2);
    }

    let head_before = git.rev_parse_head().ok();
    git.run_git_commit(args.message.as_deref(), &args.git_args)?;
    let head_after = git
        .rev_parse_head()
        .context("failed to read new HEAD after commit")?;
    if head_before.as_deref() == Some(&head_after) {
        return Err(anyhow!("git commit did not create a new commit"));
    }

    transcript.commit = Some(head_after.clone());
    let store = TranscriptStore::git_notes();
    if let Err(err) = store.store(&git.repo, &head_after, &transcript) {
        eprintln!("aigit: failed to store transcript: {err}");
        return Ok(4);
    }

    eprintln!("aigit: stored transcript in git notes for {head_after}");
    Ok(0)
}

