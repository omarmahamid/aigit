use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::git::GitRepo;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexCliPolicy {
    /// Base command used to invoke Codex CLI (no subcommand).
    ///
    /// Examples:
    /// - "codex"
    /// - "npx -y @openai/codex@0.75.0"
    #[serde(default)]
    pub command: Option<String>,

    /// Optional Codex config profile name (from ~/.codex/config.toml).
    #[serde(default)]
    pub profile: Option<String>,

    /// Optional model override (passed to `codex exec --model`).
    #[serde(default)]
    pub model: Option<String>,

    /// Sandbox mode passed to `codex exec --sandbox` (e.g. "read-only").
    #[serde(default)]
    pub sandbox: Option<String>,

    /// Timeout for the Codex process in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub min_total_score: f64,
    #[serde(default)]
    pub required_categories: Vec<String>,
    #[serde(default)]
    pub max_hallucination_flags: u32,

    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub exam_mode: Option<String>,

    #[serde(default)]
    pub store: Option<String>,

    #[serde(default)]
    pub redactions: Vec<String>,
    #[serde(default)]
    pub max_tokens_context: Option<usize>,

    #[serde(default)]
    pub hooks: Hooks,

    /// Settings used when `provider = "codex-cli"`.
    #[serde(default)]
    pub codex_cli: CodexCliPolicy,

    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hooks {
    #[serde(default)]
    pub enforce: Option<bool>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            min_total_score: 0.75,
            required_categories: vec![
                "risk".to_string(),
                "rollback".to_string(),
                "testing".to_string(),
            ],
            max_hallucination_flags: 0,
            provider: Some("local".to_string()),
            model: Some("static".to_string()),
            exam_mode: Some("tui".to_string()),
            store: Some("git-notes".to_string()),
            redactions: vec![],
            max_tokens_context: Some(4096),
            hooks: Hooks { enforce: None },
            codex_cli: CodexCliPolicy::default(),
            extra: BTreeMap::new(),
        }
    }
}

impl Policy {
    pub fn load_from_repo(repo: &GitRepo) -> Result<Self> {
        let path = repo.workdir.join(".aigit.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let policy: Self =
            toml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(policy.with_defaults())
    }

    fn with_defaults(mut self) -> Self {
        let d = Self::default();
        if self.min_total_score == 0.0 {
            self.min_total_score = d.min_total_score;
        }
        if self.required_categories.is_empty() {
            self.required_categories = d.required_categories;
        }
        if self.max_tokens_context.is_none() {
            self.max_tokens_context = d.max_tokens_context;
        }
        if self.provider.is_none() {
            self.provider = d.provider;
        }
        if self.model.is_none() {
            self.model = d.model;
        }
        if self.exam_mode.is_none() {
            self.exam_mode = d.exam_mode;
        }
        if self.store.is_none() {
            self.store = d.store;
        }
        self
    }

    pub fn max_context_chars(&self) -> usize {
        // very rough, deterministic token->chars estimate (4 chars/token)
        self.max_tokens_context.unwrap_or(4096) * 4
    }

    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "min_total_score" => {
                self.min_total_score = value
                    .parse::<f64>()
                    .map_err(|_| anyhow!("min_total_score must be a number"))?;
                Ok(())
            }
            "max_hallucination_flags" => {
                self.max_hallucination_flags = value
                    .parse::<u32>()
                    .map_err(|_| anyhow!("max_hallucination_flags must be an integer"))?;
                Ok(())
            }
            "exam_mode" => {
                self.exam_mode = Some(value.to_string());
                Ok(())
            }
            "provider" => {
                self.provider = Some(value.to_string());
                Ok(())
            }
            "model" => {
                self.model = Some(value.to_string());
                Ok(())
            }
            "store" => {
                self.store = Some(value.to_string());
                Ok(())
            }
            _ => Err(anyhow!("unsupported key: {key}")),
        }
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}
