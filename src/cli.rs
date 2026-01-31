use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "aigit",
    version,
    about = "Proof-of-Understanding commit protocol for git"
)]
pub(crate) struct Cli {
    /// Verbose output (stderr)
    #[arg(long)]
    pub(crate) verbose: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Run a PoU exam over changes (default: staged diff)
    Exam(ExamArgs),
    /// Run PoU exam then delegate to `git commit` if passed
    Commit(CommitArgs),
    /// Verify that a commit has a valid PoU transcript
    Verify(VerifyArgs),
    /// Install git hook to enforce using `aigit commit`
    InstallHook(InstallHookArgs),
    /// Dashboard utilities (export transcripts for the web UI)
    Dashboard(DashboardArgs),
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
pub(crate) enum PolicyCmd {
    Validate,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCmd {
    Set(ConfigSetArgs),
}

#[derive(Parser, Debug)]
pub(crate) struct ExamArgs {
    /// Use staged changes (default when no range is provided)
    #[arg(long, conflicts_with = "range", default_value_t = false)]
    pub(crate) staged: bool,

    /// Diff range, e.g. HEAD~1..HEAD
    #[arg(long)]
    pub(crate) range: Option<String>,

    /// Output format
    #[arg(long, value_enum)]
    pub(crate) format: Option<ExamFormat>,

    /// Answers JSON path, or '-' for stdin (only used with --format json)
    #[arg(long)]
    pub(crate) answers: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ExamFormat {
    Tui,
    Json,
}

#[derive(Parser, Debug)]
pub(crate) struct CommitArgs {
    /// Commit message (like `git commit -m`)
    #[arg(short = 'm', long)]
    pub(crate) message: Option<String>,

    /// Pass-through args to `git commit` after `--`
    #[arg(last = true)]
    pub(crate) git_args: Vec<String>,
}

#[derive(Parser, Debug)]
pub(crate) struct VerifyArgs {
    pub(crate) commitish: String,
}

#[derive(Parser, Debug)]
pub(crate) struct InstallHookArgs {
    #[arg(long, value_enum, default_value_t = HookMode::PreCommit)]
    pub(crate) mode: HookMode,

    /// Overwrite existing hook
    #[arg(long, default_value_t = false)]
    pub(crate) force: bool,
}

#[derive(Parser, Debug)]
pub(crate) struct DashboardArgs {
    #[command(subcommand)]
    pub(crate) command: DashboardCmd,
}

#[derive(Subcommand, Debug)]
pub(crate) enum DashboardCmd {
    /// Export transcripts from git notes (ref=aigit) as JSON for the web dashboard
    Export(DashboardExportArgs),
    /// Serve the dashboard as a local static site
    Serve(DashboardServeArgs),
}

#[derive(Parser, Debug)]
pub(crate) struct DashboardExportArgs {
    /// Output path for the exported JSON
    #[arg(long, default_value = "dashboard/public/data.json")]
    pub(crate) out: String,

    /// Include full answer text in the export (can be sensitive)
    #[arg(long, default_value_t = false)]
    pub(crate) include_answers: bool,

    /// Maximum number of transcripts to export (newest first)
    #[arg(long)]
    pub(crate) limit: Option<usize>,
}

#[derive(Parser, Debug)]
pub(crate) struct DashboardServeArgs {
    /// Directory to serve (should contain index.html)
    #[arg(long, default_value = "dashboard/public")]
    pub(crate) dir: String,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub(crate) host: String,

    /// Port to bind to
    #[arg(long, default_value_t = 5173)]
    pub(crate) port: u16,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum HookMode {
    PreCommit,
}

#[derive(Parser, Debug)]
pub(crate) struct ConfigSetArgs {
    pub(crate) key: String,
    pub(crate) value: String,
}
