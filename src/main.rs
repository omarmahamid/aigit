mod config;
mod codex_cli;
mod examiner;
mod git;
mod redact;
mod transcript;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::Policy;
use crate::examiner::{CodexCliExaminer, Examiner, StaticExaminer};
use crate::git::{Git, GitRepo};
use crate::transcript::{Decision, TranscriptStore};

#[derive(Parser, Debug)]
#[command(
    name = "aigit",
    version,
    about = "Proof-of-Understanding commit protocol for git"
)]
struct Cli {
    /// Verbose output (stderr)
    #[arg(long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a PoU exam over changes (default: staged diff)
    Exam(ExamArgs),
    /// Run PoU exam then delegate to `git commit` if passed
    #[command(trailing_var_arg = true)]
    Commit(CommitArgs),
    /// Verify that a commit has a valid PoU transcript
    Verify(VerifyArgs),
    /// Install git hook to enforce using `aigit commit`
    InstallHook(InstallHookArgs),
    /// Policy utilities
    Policy {
        #[command(subcommand)]
        command: PolicyCmd,
    },
    /// Config utilities
    Config {
        #[command(subcommand)]
        command: ConfigCmd,
    },
}

#[derive(Subcommand, Debug)]
enum PolicyCmd {
    Validate,
}

#[derive(Subcommand, Debug)]
enum ConfigCmd {
    Set(ConfigSetArgs),
}

#[derive(Parser, Debug)]
struct ExamArgs {
    /// Use staged changes (default when no range is provided)
    #[arg(long, conflicts_with = "range", default_value_t = false)]
    staged: bool,

    /// Diff range, e.g. HEAD~1..HEAD
    #[arg(long)]
    range: Option<String>,

    /// Output format
    #[arg(long, value_enum)]
    format: Option<ExamFormat>,

    /// Answers JSON path, or '-' for stdin (only used with --format json)
    #[arg(long)]
    answers: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExamFormat {
    Tui,
    Json,
}

#[derive(Parser, Debug)]
struct CommitArgs {
    /// Commit message (like `git commit -m`)
    #[arg(short = 'm', long)]
    message: Option<String>,

    /// Pass-through args to `git commit` after `--`
    #[arg(last = true)]
    git_args: Vec<String>,
}

#[derive(Parser, Debug)]
struct VerifyArgs {
    commitish: String,
}

#[derive(Parser, Debug)]
struct InstallHookArgs {
    #[arg(long, value_enum, default_value_t = HookMode::PreCommit)]
    mode: HookMode,

    /// Overwrite existing hook
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum HookMode {
    PreCommit,
}

#[derive(Parser, Debug)]
struct ConfigSetArgs {
    key: String,
    value: String,
}

fn main() -> ExitCode {
    ExitCode::from(run())
}

fn run() -> u8 {
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
        Commands::Exam(args) => cmd_exam(&git, args, cli.verbose),
        Commands::Commit(args) => cmd_commit(&git, args, cli.verbose),
        Commands::Verify(args) => cmd_verify(&git, args, cli.verbose),
        Commands::InstallHook(args) => cmd_install_hook(&git, args),
        Commands::Policy { command } => match command {
            PolicyCmd::Validate => {
                let policy = Policy::load_from_repo(&git.repo)?;
                if cli.verbose {
                    eprintln!("policy: {policy:#?}");
                }
                Ok(0)
            }
        },
        Commands::Config { command } => match command {
            ConfigCmd::Set(args) => cmd_config_set(&git, args),
        },
    }
}

fn cmd_exam(git: &Git, args: ExamArgs, verbose: bool) -> Result<u8> {
    let policy = Policy::load_from_repo(&git.repo)?;
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
    let (redacted_diff, redactions) = redact::redact_diff(&policy, &diff)?;
    let ctx = examiner::ExamContext::new(
        &git,
        diff_patch_id,
        &redacted_diff,
        changed_files,
        redactions,
        &policy,
    )?;

    let examiner: Box<dyn Examiner> = match policy.provider.as_deref() {
        Some("codex-cli") => Box::new(CodexCliExaminer::new(&policy)),
        _ => Box::new(StaticExaminer::new()),
    };
    let exam = examiner.generate_exam(&ctx)?;

    match format {
        ExamFormat::Json => {
            if let Some(path) = args.answers {
                let answers = transcript::Answers::load_from_path(&path)?;
                let score = examiner.grade_exam(&ctx, &exam, &answers)?;
                let decision = transcript::Decision::from_score(&policy, &exam, &answers, &score);
                let transcript = transcript::Transcript::from_exam_result(
                    &git, &policy, &ctx, &exam, &answers, &score, decision,
                )?;
                serde_json::to_writer_pretty(std::io::stdout(), &transcript)?;
                println!();
                Ok(match transcript.decision {
                    Decision::Pass => 0,
                    Decision::Fail => 2,
                })
            } else {
                let packet = examiner::ExamPacket::from_context(&ctx, exam);
                serde_json::to_writer_pretty(std::io::stdout(), &packet)?;
                println!();
                Ok(0)
            }
        }
        ExamFormat::Tui => {
            if verbose {
                eprintln!("changed files: {:?}", ctx.changed_files);
            }
            let answers = transcript::Answers::prompt_tui(&exam)?;
            let score = examiner.grade_exam(&ctx, &exam, &answers)?;
            let decision = transcript::Decision::from_score(&policy, &exam, &answers, &score);
            let transcript = transcript::Transcript::from_exam_result(
                &git, &policy, &ctx, &exam, &answers, &score, decision,
            )?;
            transcript::print_human_result(&transcript);
            Ok(match transcript.decision {
                Decision::Pass => 0,
                Decision::Fail => 2,
            })
        }
    }
}

fn cmd_commit(git: &Git, args: CommitArgs, verbose: bool) -> Result<u8> {
    let policy = Policy::load_from_repo(&git.repo)?;
    let (diff, changed_files) = git.diff_staged()?;
    if diff.trim().is_empty() {
        return Err(anyhow!("no staged changes to commit"));
    }

    let diff_patch_id = git.patch_id_from_diff_text(&diff)?;
    let (redacted_diff, redactions) = redact::redact_diff(&policy, &diff)?;
    let ctx = examiner::ExamContext::new(
        git,
        diff_patch_id,
        &redacted_diff,
        changed_files,
        redactions,
        &policy,
    )?;

    let examiner: Box<dyn Examiner> = match policy.provider.as_deref() {
        Some("codex-cli") => Box::new(CodexCliExaminer::new(&policy)),
        _ => Box::new(StaticExaminer::new()),
    };
    let exam = examiner.generate_exam(&ctx)?;
    let answers = transcript::Answers::prompt_tui(&exam)?;
    let score = examiner.grade_exam(&ctx, &exam, &answers)?;
    let decision = transcript::Decision::from_score(&policy, &exam, &answers, &score);

    let mut transcript = transcript::Transcript::from_exam_result(
        git, &policy, &ctx, &exam, &answers, &score, decision,
    )?;

    if verbose {
        eprintln!("exam decision: {:?}", transcript.decision);
    }
    transcript::print_human_result(&transcript);
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

fn cmd_verify(git: &Git, args: VerifyArgs, _verbose: bool) -> Result<u8> {
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

fn cmd_install_hook(git: &Git, args: InstallHookArgs) -> Result<u8> {
    match args.mode {
        HookMode::PreCommit => {
            git.install_pre_commit_hook(args.force)?;
            Ok(0)
        }
    }
}

fn cmd_config_set(git: &Git, args: ConfigSetArgs) -> Result<u8> {
    let mut policy = Policy::load_from_repo(&git.repo)?;
    policy.set_key(&args.key, &args.value)?;
    let path: PathBuf = git.repo.workdir.join(".aigit.toml");
    std::fs::write(&path, policy.to_toml_string()?)?;
    println!("wrote {}", path.display());
    Ok(0)
}
