use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use wait_timeout::ChildExt;

use crate::config::{CodexCliPolicy, Policy};

pub const NPX_OPENAI_DOWNLOAD: &str = "npx -y @openai/codex@0.93.0";

#[derive(Debug, Clone)]
pub struct CodexCliRunner {
    base_command: String,
    profile: Option<String>,
    model: Option<String>,
    sandbox: String,
    timeout: Duration,
}

impl CodexCliRunner {
    pub fn from_policy(policy: &Policy) -> Self {
        let cfg: &CodexCliPolicy = &policy.codex_cli;
        let base_command = cfg
            .command
            .clone()
            .unwrap_or_else(|| "codex".to_string());
        let sandbox = cfg
            .sandbox
            .clone()
            .unwrap_or_else(|| "read-only".to_string());
        let timeout = Duration::from_secs(cfg.timeout_secs.unwrap_or(120));
        Self {
            base_command,
            profile: cfg.profile.clone(),
            model: cfg.model.clone().or_else(|| policy.model.clone()),
            sandbox,
            timeout,
        }
    }

    pub fn run_json_judge(&self, cwd: &Path, prompt: &str) -> Result<String> {
        self.run_json_with_schema(cwd, prompt, &score_schema_json())
    }

    pub fn run_json_generate_exam(&self, cwd: &Path, prompt: &str) -> Result<String> {
        self.run_json_with_schema(cwd, prompt, &exam_schema_json())
    }

    fn run_json_with_schema(
        &self,
        cwd: &Path,
        prompt: &str,
        schema: &serde_json::Value,
    ) -> Result<String> {
        let tmp = tempfile::tempdir().context("failed to create temp dir for codex judge")?;
        let schema_path = tmp.path().join("aigit-codex-judge.schema.json");
        let output_path = tmp.path().join("aigit-codex-judge.output.json");

        std::fs::write(&schema_path, serde_json::to_vec_pretty(schema)?)
            .with_context(|| format!("failed to write {}", schema_path.display()))?;

        let (program, mut args) = split_command_line(&self.base_command)?;
        // Base command is expected to be a Codex CLI invocation (e.g. "codex" or "npx … @openai/codex@…").
        // If the user already included the subcommand, do not append it again.
        if !args.iter().any(|a| a == "exec") {
            args.push("exec".to_string());
        }

        if let Some(profile) = &self.profile {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }
        if let Some(model) = &self.model {
            if model != "static" {
                args.push("--model".to_string());
                args.push(model.clone());
            }
        }

        args.extend([
            "--color".to_string(),
            "never".to_string(),
            "--sandbox".to_string(),
            self.sandbox.clone(),
            "--output-schema".to_string(),
            schema_path.display().to_string(),
            "--output-last-message".to_string(),
            output_path.display().to_string(),
            "-".to_string(),
        ]);

        let mut cmd = Command::new(&program);
        cmd.current_dir(cwd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "error");

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "failed to spawn Codex CLI: {} {} (hint: set `codex_cli.command` in .aigit.toml, e.g. \"{}\")",
                program,
                args.join(" "),
                NPX_OPENAI_DOWNLOAD
            )
        })?;

        {
            use std::io::Write;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("codex exec missing stdin"))?;
            stdin
                .write_all(prompt.as_bytes())
                .context("failed to write prompt to codex stdin")?;
        }

        let stdout_handle = child.stdout.take().map(read_to_end_thread);
        let stderr_handle = child.stderr.take().map(read_to_end_thread);

        let status = match child.wait_timeout(self.timeout)? {
            Some(s) => s,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!(
                    "codex exec timed out after {}s",
                    self.timeout.as_secs()
                ));
            }
        };

        let stdout = stdout_handle
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr = stderr_handle
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();

        if !status.success() {
            return Err(anyhow!(
                "codex exec failed (exit={})\nstdout:\n{}\nstderr:\n{}",
                status,
                truncate_for_error(&stdout),
                truncate_for_error(&stderr)
            ));
        }

        let raw = std::fs::read_to_string(&output_path)
            .with_context(|| format!("codex exec did not write {}", output_path.display()))?;
        Ok(raw)
    }
}

fn read_to_end_thread(mut reader: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    })
}

fn truncate_for_error(s: &str) -> String {
    const MAX: usize = 8000;
    if s.len() <= MAX {
        return s.to_string();
    }
    let mut out = s[..MAX].to_string();
    out.push_str("\n[aigit: output truncated]\n");
    out
}

fn split_command_line(input: &str) -> Result<(String, Vec<String>)> {
    let parts = shlex::split(input).ok_or_else(|| anyhow!("invalid base command: {input}"))?;
    if parts.is_empty() {
        return Err(anyhow!("base command is empty"));
    }
    let mut parts_iter = parts.into_iter();
    let program = parts_iter.next().unwrap();
    Ok((program, parts_iter.collect()))
}

fn score_schema_json() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "aigit.Score",
        "type": "object",
        "additionalProperties": false,
        "required": ["total_score", "per_question", "hallucination_flags"],
        "properties": {
            "total_score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "hallucination_flags": {
                "type": "array",
                "items": { "type": "string" }
            },
            "per_question": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "category", "score", "completeness", "specificity", "notes"],
                    "properties": {
                        "id": { "type": "string" },
                        "category": { "type": "string" },
                        "score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "completeness": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "specificity": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "notes": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }
        }
    })
}

fn exam_schema_json() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "aigit.Exam",
        "type": "object",
        "additionalProperties": false,
        "required": ["protocol_version", "questions"],
        "properties": {
            "protocol_version": { "type": "string" },
            "questions": {
                "type": "array",
                "minItems": 4,
                "maxItems": 12,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    // OpenAI/Codex schema validation requires `required` to list every key in `properties`.
                    // So `choices` is required but may be null for open-ended questions.
                    "required": ["id", "category", "prompt", "choices"],
                    "properties": {
                        "id": { "type": "string" },
                        "category": { "type": "string" },
                        "prompt": { "type": "string" },
                        "choices": {
                            "type": ["array", "null"],
                            "minItems": 2,
                            "maxItems": 6,
                            "items": { "type": "string" }
                        }
                    }
                }
            }
        }
    })
}
