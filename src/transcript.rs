use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Policy;
use crate::examiner::{Exam, ExamContext};
use crate::git::{Git, GitRepo};
use crate::redact::RedactionHit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answers {
    pub answers: BTreeMap<String, String>,
}

impl Answers {
    pub fn get(&self, id: &str) -> Option<&str> {
        self.answers.get(id).map(|s| s.as_str())
    }

    pub fn load_from_path(path: &str) -> Result<Self> {
        if path == "-" {
            let mut buf = String::new();
            use std::io::Read;
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(serde_json::from_str(&buf)?)
        } else {
            let raw = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&raw)?)
        }
    }

    pub fn prompt_tui(exam: &Exam) -> Result<Self> {
        let mut answers = BTreeMap::new();
        println!("aigit exam: answer the following questions.\n");
        for q in &exam.questions {
            println!("--- [{}] {} ---", q.category, q.prompt);
            println!("(end your answer with a single '.' on its own line)\n");
            let text = read_multiline_until_dot()?;
            answers.insert(q.id.clone(), text);
            println!();
        }
        Ok(Self { answers })
    }
}

fn read_multiline_until_dot() -> Result<String> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut out = String::new();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim() == "." {
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionScore {
    pub id: String,
    pub category: String,
    pub score: f64,
    pub completeness: f64,
    pub specificity: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub total_score: f64,
    pub per_question: Vec<QuestionScore>,
    pub hallucination_flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Pass,
    Fail,
}

impl Decision {
    pub fn from_score(policy: &Policy, exam: &Exam, answers: &Answers, score: &Score) -> Self {
        if score.total_score < policy.min_total_score {
            return Decision::Fail;
        }
        if (score.hallucination_flags.len() as u32) > policy.max_hallucination_flags {
            return Decision::Fail;
        }
        for cat in &policy.required_categories {
            let required_answered = exam
                .questions
                .iter()
                .filter(|q| q.category == *cat)
                .all(|q| answers.get(&q.id).unwrap_or("").trim().len() > 0);
            if !required_answered {
                return Decision::Fail;
            }
        }
        Decision::Pass
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFingerprint {
    pub patch_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub schema_version: String,
    pub commit: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub repo_id: String,
    pub repo_fingerprint: String,
    pub diff_fingerprint: DiffFingerprint,
    pub exam: Exam,
    pub answers: Answers,
    pub score: Score,
    pub decision: Decision,
    pub thresholds: PolicyThresholds,
    pub provider: ProviderMetadata,
    pub redactions: Vec<RedactionHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyThresholds {
    pub min_total_score: f64,
    pub required_categories: Vec<String>,
    pub max_hallucination_flags: u32,
}

impl Transcript {
    pub fn from_exam_result(
        _git: &Git,
        policy: &Policy,
        ctx: &ExamContext,
        exam: &Exam,
        answers: &Answers,
        score: &Score,
        decision: Decision,
    ) -> Result<Self> {
        let repo_fingerprint = fingerprint_repo(&ctx.repo_id);
        Ok(Self {
            schema_version: "aigit-transcript/0.1".to_string(),
            commit: None,
            timestamp: Utc::now(),
            repo_id: ctx.repo_id.clone(),
            repo_fingerprint,
            diff_fingerprint: DiffFingerprint {
                patch_id: ctx.diff_patch_id.clone(),
            },
            exam: exam.clone(),
            answers: answers.clone(),
            score: score.clone(),
            decision,
            thresholds: PolicyThresholds {
                min_total_score: policy.min_total_score,
                required_categories: policy.required_categories.clone(),
                max_hallucination_flags: policy.max_hallucination_flags,
            },
            provider: ProviderMetadata {
                provider: policy
                    .provider
                    .clone()
                    .unwrap_or_else(|| "local".to_string()),
                model: policy.model.clone().unwrap_or_else(|| "static".to_string()),
                prompt_version: "static/0.1".to_string(),
            },
            redactions: ctx.redactions.clone(),
        })
    }

    pub fn verify_against_policy(&self, policy: &Policy) -> bool {
        if self.decision != Decision::Pass {
            return false;
        }
        if self.score.total_score < policy.min_total_score {
            return false;
        }
        if (self.score.hallucination_flags.len() as u32) > policy.max_hallucination_flags {
            return false;
        }
        for cat in &policy.required_categories {
            let ok = self
                .exam
                .questions
                .iter()
                .filter(|q| q.category == *cat)
                .all(|q| self.answers.get(&q.id).unwrap_or("").trim().len() > 0);
            if !ok {
                return false;
            }
        }
        true
    }
}

fn fingerprint_repo(repo_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(repo_id.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

pub fn print_human_result(t: &Transcript) {
    match t.decision {
        Decision::Pass => {
            eprintln!("aigit: PASS (score {:.2})", t.score.total_score);
        }
        Decision::Fail => {
            eprintln!("aigit: FAIL (score {:.2})", t.score.total_score);
            if !t.score.hallucination_flags.is_empty() {
                eprintln!("aigit: hallucination flags:");
                for f in &t.score.hallucination_flags {
                    eprintln!("  - {f}");
                }
            }
        }
    }
}

pub struct TranscriptStore {
    kind: StoreKind,
}

enum StoreKind {
    GitNotes,
}

impl TranscriptStore {
    pub fn git_notes() -> Self {
        Self {
            kind: StoreKind::GitNotes,
        }
    }

    pub fn store(&self, repo: &GitRepo, commit: &str, transcript: &Transcript) -> Result<()> {
        match self.kind {
            StoreKind::GitNotes => git_notes_store(repo, commit, transcript),
        }
    }

    pub fn load(&self, repo: &GitRepo, commit: &str) -> Result<Transcript> {
        match self.kind {
            StoreKind::GitNotes => git_notes_load(repo, commit),
        }
    }
}

fn git_notes_store(repo: &GitRepo, commit: &str, transcript: &Transcript) -> Result<()> {
    let json = serde_json::to_string_pretty(transcript)?;
    let status = std::process::Command::new("git")
        .current_dir(&repo.workdir)
        .args(["notes", "--ref=aigit", "add", "-f", "-m", &json, commit])
        .status()
        .context("failed to run git notes add")?;
    if !status.success() {
        return Err(anyhow!("git notes add failed"));
    }
    Ok(())
}

fn git_notes_load(repo: &GitRepo, commit: &str) -> Result<Transcript> {
    let out = std::process::Command::new("git")
        .current_dir(&repo.workdir)
        .args(["notes", "--ref=aigit", "show", commit])
        .output()
        .context("failed to run git notes show")?;
    if !out.status.success() {
        return Err(anyhow!("no transcript found in git notes for {commit}"));
    }
    let raw = String::from_utf8(out.stdout)?;
    let t: Transcript = serde_json::from_str(&raw)
        .with_context(|| "failed to parse transcript JSON from git notes")?;
    if t.schema_version != "aigit-transcript/0.1" {
        return Err(anyhow!(
            "unsupported transcript schema {}",
            t.schema_version
        ));
    }
    Ok(t)
}
