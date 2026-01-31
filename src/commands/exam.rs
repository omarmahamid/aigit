use anyhow::{anyhow, Result};

use crate::cli::{ExamArgs, ExamFormat};
use crate::examiner::{ExamContext, ExamPacket, Examiner};
use crate::git::Git;
use crate::transcript::Decision;

use super::common;

pub(crate) fn cmd_exam(git: &Git, args: ExamArgs, verbose: bool) -> Result<u8> {
    let policy = common::load_policy_verbose(git, verbose)?;

    let format = match args.format {
        Some(ExamFormat::Tui) => ExamFormat::Tui,
        Some(ExamFormat::Json) => ExamFormat::Json,
        None => match policy.exam_mode.as_deref() {
            Some("json") => ExamFormat::Json,
            _ => ExamFormat::Tui,
        },
    };

    let (diff, changed_files) = if let Some(range) = args.range {
        git.diff_range(&range)?
    } else if args.staged {
        git.diff_staged()?
    } else {
        // default
        git.diff_staged()?
    };

    if diff.trim().is_empty() {
        return Err(anyhow!("no changes to examine (diff is empty)"));
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

    match format {
        ExamFormat::Json => {
            if let Some(path) = args.answers {
                let answers = crate::transcript::Answers::load_from_path(&path)?;
                let score = examiner.grade_exam(&ctx, &exam, &answers)?;
                let decision = crate::transcript::Decision::from_score(&policy, &exam, &answers, &score);
                let transcript = crate::transcript::Transcript::from_exam_result(
                    git, &policy, &ctx, &exam, &answers, &score, decision,
                )?;
                serde_json::to_writer_pretty(std::io::stdout(), &transcript)?;
                println!();
                Ok(match transcript.decision {
                    Decision::Pass => 0,
                    Decision::Fail => 2,
                })
            } else {
                let packet = ExamPacket::from_context(&ctx, exam);
                serde_json::to_writer_pretty(std::io::stdout(), &packet)?;
                println!();
                Ok(0)
            }
        }
        ExamFormat::Tui => {
            if verbose {
                eprintln!("changed files: {:?}", ctx.changed_files);
            }
            let answers = crate::transcript::Answers::prompt_tui(&exam)?;
            let score = examiner.grade_exam(&ctx, &exam, &answers)?;
            let decision = crate::transcript::Decision::from_score(&policy, &exam, &answers, &score);
            let transcript = crate::transcript::Transcript::from_exam_result(
                git, &policy, &ctx, &exam, &answers, &score, decision,
            )?;
            crate::transcript::print_human_result(&transcript);
            Ok(match transcript.decision {
                Decision::Pass => 0,
                Decision::Fail => 2,
            })
        }
    }
}
