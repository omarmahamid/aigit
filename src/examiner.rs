use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Policy;
use crate::codex_cli::CodexCliRunner;
use crate::git::Git;
use crate::redact::RedactionHit;
use crate::transcript::{Answers, Score};

const KEYWORDS_RISK: &[&str] = &["risk", "break", "fail", "regress", "error", "panic"];
const KEYWORDS_TESTING: &[&str] = &["test", "cargo test", "unit", "integration", "ci"];
const KEYWORDS_ROLLBACK: &[&str] = &["revert", "rollback", "backout", "feature flag", "mitigate"];
const KEYWORDS_SECURITY: &[&str] = &["auth", "authz", "pii", "secret", "token", "key", "encrypt"];
const KEYWORDS_DEFAULT: &[&str] = &["file", "module", "function", "line"];

#[derive(Debug, Clone)]
pub struct ExamContext {
    pub repo_id: String,
    pub workdir: std::path::PathBuf,
    pub diff_patch_id: String,
    #[allow(dead_code)]
    pub diff: String,
    pub changed_files: Vec<String>,
    pub redactions: Vec<RedactionHit>,
    #[allow(dead_code)]
    pub policy: Policy,
}

impl ExamContext {
    pub fn new(
        git: &Git,
        diff_patch_id: String,
        diff_redacted: &str,
        changed_files: Vec<String>,
        redactions: Vec<RedactionHit>,
        policy: &Policy,
    ) -> Result<Self> {
        let repo_id = git
            .remote_fingerprint()?
            .unwrap_or_else(|| git.repo.workdir.display().to_string());
        let mut diff = diff_redacted.to_string();
        let max_chars = policy.max_context_chars();
        if diff.len() > max_chars {
            diff.truncate(max_chars);
            diff.push_str("\n\n[aigit: diff truncated]\n");
        }
        Ok(Self {
            repo_id,
            workdir: git.repo.workdir.clone(),
            diff_patch_id,
            diff,
            changed_files,
            redactions,
            policy: policy.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamQuestion {
    pub id: String,
    pub category: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exam {
    pub protocol_version: String,
    pub questions: Vec<ExamQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamPacket {
    pub schema_version: String,
    pub repo_id: String,
    pub diff_patch_id: String,
    pub changed_files: Vec<String>,
    pub diff_redacted: String,
    pub redactions: Vec<RedactionHit>,
    pub exam: Exam,
}

impl ExamPacket {
    pub fn from_context(ctx: &ExamContext, exam: Exam) -> Self {
        Self {
            schema_version: "aigit-exam/0.1".to_string(),
            repo_id: ctx.repo_id.clone(),
            diff_patch_id: ctx.diff_patch_id.clone(),
            changed_files: ctx.changed_files.clone(),
            diff_redacted: ctx.diff.clone(),
            redactions: ctx.redactions.clone(),
            exam,
        }
    }
}

pub trait Examiner {
    fn generate_exam(&self, ctx: &ExamContext) -> Result<Exam>;
    fn grade_exam(&self, ctx: &ExamContext, exam: &Exam, answers: &Answers) -> Result<Score>;
}

#[derive(Debug, Clone)]
pub struct StaticExaminer;

impl StaticExaminer {
    pub fn new() -> Self {
        Self
    }
}

impl Examiner for StaticExaminer {
    fn generate_exam(&self, _ctx: &ExamContext) -> Result<Exam> {
        let questions = vec![
            ExamQuestion {
                id: "change_summary".to_string(),
                category: "summary".to_string(),
                prompt: "Summarize what changed (concrete files/modules) and why.".to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "intent".to_string(),
                category: "intent".to_string(),
                prompt: "What user/business requirement does this satisfy?".to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "invariants".to_string(),
                category: "invariants".to_string(),
                prompt: "What assumptions does this change rely on? What invariants must remain true?"
                    .to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "risk".to_string(),
                category: "risk".to_string(),
                prompt: "What could break, and where would issues surface first (blast radius)?"
                    .to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "testing".to_string(),
                category: "testing".to_string(),
                prompt: "What tests were run? Which should exist? What coverage is missing?".to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "rollback".to_string(),
                category: "rollback".to_string(),
                prompt: "How would you rollback/revert/mitigate if this change causes problems?"
                    .to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "alternatives".to_string(),
                category: "alternatives".to_string(),
                prompt: "What alternative approach was considered, and why was it rejected?"
                    .to_string(),
                choices: None,
            },
            ExamQuestion {
                id: "security_privacy".to_string(),
                category: "security".to_string(),
                prompt: "Any security/privacy concerns (auth/authz, PII, secrets, data access)? If not relevant, explain why."
                    .to_string(),
                choices: None,
            },
        ];
        Ok(Exam {
            protocol_version: "aigit/0.1".to_string(),
            questions,
        })
    }

    fn grade_exam(&self, ctx: &ExamContext, exam: &Exam, answers: &Answers) -> Result<Score> {
        let mut per_question = Vec::new();
        let mut hallucination_flags = Vec::new();

        for q in &exam.questions {
            let answer = answers.get(&q.id).unwrap_or_default().trim().to_string();
            let mut notes = Vec::new();
            let completeness = if answer.is_empty() { 0.0 } else { 1.0 };
            if completeness == 0.0 {
                notes.push("empty answer".to_string());
            }

            let mentions_changed_file = ctx
                .changed_files
                .iter()
                .any(|f| !f.is_empty() && answer.contains(f));
            if completeness > 0.0 && !mentions_changed_file && !ctx.changed_files.is_empty() {
                notes.push("does not mention any changed file path".to_string());
            }

            let word_count = answer.split_whitespace().count();
            if completeness > 0.0 && word_count < 20 {
                notes.push(format!("answer is short ({word_count} words)"));
            }
            let specificity = if answer.is_empty() {
                0.0
            } else if mentions_changed_file {
                1.0
            } else if word_count >= 20 {
                0.6
            } else {
                0.3
            };

            let expected_keywords = match q.category.as_str() {
                "risk" => KEYWORDS_RISK,
                "testing" => KEYWORDS_TESTING,
                "rollback" => KEYWORDS_ROLLBACK,
                "security" => KEYWORDS_SECURITY,
                _ => KEYWORDS_DEFAULT,
            };
            let category_bonus = keyword_score(&answer, expected_keywords);
            if completeness > 0.0 && category_bonus <= 0.2 {
                notes.push(format!(
                    "missing category signals (look for: {})",
                    expected_keywords.join(", ")
                ));
            }

            if completeness > 0.0 {
                // very conservative "hallucination": explicit file paths not in changed set
                for mentioned in extract_file_like_tokens(&answer) {
                    if !ctx.changed_files.iter().any(|f| f == &mentioned) {
                        hallucination_flags.push(format!(
                            "{}: mentions file not in diff: {}",
                            q.id, mentioned
                        ));
                    }
                }
            }

            let score = 0.4 * completeness + 0.4 * specificity + 0.2 * category_bonus;
            per_question.push(crate::transcript::QuestionScore {
                id: q.id.clone(),
                category: q.category.clone(),
                score,
                completeness,
                specificity,
                notes,
            });
        }

        let total_score = if per_question.is_empty() {
            0.0
        } else {
            per_question.iter().map(|q| q.score).sum::<f64>() / (per_question.len() as f64)
        };

        Ok(Score {
            total_score,
            per_question,
            hallucination_flags,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodexCliExaminer {
    runner: CodexCliRunner,
}

impl CodexCliExaminer {
    pub fn new(policy: &Policy) -> Self {
        Self {
            runner: CodexCliRunner::from_policy(policy),
        }
    }
}

impl Examiner for CodexCliExaminer {
    fn generate_exam(&self, ctx: &ExamContext) -> Result<Exam> {
        let prompt = build_codex_cli_generate_exam_prompt(ctx);
        let raw = self
            .runner
            .run_json_generate_exam(&ctx.workdir, &prompt)?;

        let mut exam: Exam = serde_json::from_str(&raw)?;
        if exam.protocol_version.trim().is_empty() {
            exam.protocol_version = "aigit/0.1".to_string();
        }
        // Basic sanity: unique ids.
        let mut ids = std::collections::BTreeSet::new();
        for q in &exam.questions {
            if q.id.trim().is_empty() {
                return Err(anyhow::anyhow!("codex exam question id is empty"));
            }
            if !ids.insert(q.id.clone()) {
                return Err(anyhow::anyhow!(
                    "codex exam contains duplicate question id: {}",
                    q.id
                ));
            }
        }
        Ok(exam)
    }

    fn grade_exam(&self, ctx: &ExamContext, exam: &Exam, answers: &Answers) -> Result<Score> {
        let prompt = build_codex_cli_judge_prompt(ctx, exam, answers);
        let raw = self
            .runner
            .run_json_judge(&ctx.workdir, &prompt)?;

        let mut score: Score = serde_json::from_str(&raw)?;

        // Validate that the response covers exactly the current exam questions.
        let expected_ids: std::collections::BTreeSet<&str> =
            exam.questions.iter().map(|q| q.id.as_str()).collect();
        let got_ids: std::collections::BTreeSet<&str> =
            score.per_question.iter().map(|q| q.id.as_str()).collect();
        if expected_ids != got_ids {
            return Err(anyhow::anyhow!(
                "codex judge returned mismatched question ids (expected {:?}, got {:?})",
                expected_ids,
                got_ids
            ));
        }

        // Defensive: clamp scores into [0,1] so policy checks behave.
        score.total_score = clamp01(score.total_score);
        for q in &mut score.per_question {
            q.score = clamp01(q.score);
            q.completeness = clamp01(q.completeness);
            q.specificity = clamp01(q.specificity);
        }

        // Keep the existing conservative hallucination flags (file mentions not in changed set).
        // Merge with the model-provided flags.
        let mut conservative = Vec::new();
        for q in &exam.questions {
            let answer = answers.get(&q.id).unwrap_or_default().trim().to_string();
            if answer.is_empty() {
                continue;
            }
            for mentioned in extract_file_like_tokens(&answer) {
                if !ctx.changed_files.iter().any(|f| f == &mentioned) {
                    conservative.push(format!(
                        "{}: mentions file not in diff: {}",
                        q.id, mentioned
                    ));
                }
            }
        }
        score.hallucination_flags.extend(conservative);
        score.hallucination_flags.sort();
        score.hallucination_flags.dedup();

        Ok(score)
    }
}

fn keyword_score(answer: &str, keywords: &[&str]) -> f64 {
    if answer.trim().is_empty() {
        return 0.0;
    }
    let lower = answer.to_lowercase();
    let hits = keywords
        .iter()
        .filter(|k| lower.contains(&k.to_lowercase()))
        .count();
    if hits >= 2 {
        1.0
    } else if hits == 1 {
        0.6
    } else {
        0.2
    }
}

fn clamp01(v: f64) -> f64 {
    if v.is_nan() {
        return 0.0;
    }
    v.max(0.0).min(1.0)
}

fn extract_file_like_tokens(answer: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in answer.split_whitespace() {
        let token = token.trim_matches(|c: char| {
            c == ','
                || c == '.'
                || c == ';'
                || c == ')'
                || c == '('
                || c == '"'
                || c == '\''
                || c == '`'
        });
        if token.contains('/') && token.contains('.') && token.len() <= 120 {
            out.push(token.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn build_codex_cli_judge_prompt(ctx: &ExamContext, exam: &Exam, answers: &Answers) -> String {
    let mut out = String::new();
    out.push_str("You are a strict grader for a git \"Proof-of-Understanding\" exam.\n");
    out.push_str("Use ONLY the provided context; do not run commands, read files, or assume details not present.\n");
    out.push_str("Return ONLY a JSON object matching the provided JSON Schema.\n\n");

    out.push_str("Grading rubric:\n");
    out.push_str("- completeness: 0..1 based on how well the answer addresses the question (0 if empty).\n");
    out.push_str("- specificity: 0..1 based on concrete references to what changed (files/functions/behaviors in the diff), not generic boilerplate.\n");
    out.push_str("- score: 0..1 overall for the question; recommended weighting: 0.45*completeness + 0.45*specificity + 0.10*category_relevance.\n");
    out.push_str("- notes: short bullet-like strings explaining missing specifics or inaccuracies.\n");
    out.push_str("- hallucination_flags: conservative flags for claims not supported by the diff (esp. files/modules not in changed_files).\n\n");

    out.push_str("changed_files:\n");
    for f in &ctx.changed_files {
        out.push_str("- ");
        out.push_str(f);
        out.push('\n');
    }
    out.push('\n');

    out.push_str("diff_redacted (may be truncated):\n");
    out.push_str("-----\n");
    out.push_str(&ctx.diff);
    out.push_str("\n-----\n\n");

    out.push_str("questions_and_answers:\n");
    for q in &exam.questions {
        let a = answers.get(&q.id).unwrap_or_default().trim();
        out.push_str(&format!("\n[id={}] [category={}] prompt: {}\n", q.id, q.category, q.prompt));
        out.push_str("answer:\n");
        out.push_str(a);
        out.push('\n');
    }
    out
}

fn build_codex_cli_generate_exam_prompt(ctx: &ExamContext) -> String {
    let mut out = String::new();
    out.push_str("You generate a git \"Proof-of-Understanding\" exam tailored to a specific diff.\n");
    out.push_str("Use ONLY the provided context; do not run commands, read files, or assume details not present.\n");
    out.push_str("Return ONLY a JSON object matching the provided JSON Schema.\n\n");

    out.push_str("Requirements:\n");
    out.push_str("- 8 questions total (unless the diff is tiny; then >=4).\n");
    out.push_str("- Cover these categories at least once each: summary, intent, invariants, risk, testing, rollback, alternatives, security.\n");
    out.push_str("- Make questions diff-aware: mention concrete files/functions/behaviors present in the diff.\n");
    out.push_str("- Include at least 3 multiple-choice questions by providing a `choices` array with 4 options.\n");
    out.push_str("- Multiple-choice questions should be answerable with A/B/C/D.\n");
    out.push_str("- At least one question should probe an alternative approach and ask why it was not chosen.\n\n");

    out.push_str("changed_files:\n");
    for f in &ctx.changed_files {
        out.push_str("- ");
        out.push_str(f);
        out.push('\n');
    }
    out.push('\n');

    out.push_str("diff_redacted (may be truncated):\n");
    out.push_str("-----\n");
    out.push_str(&ctx.diff);
    out.push_str("\n-----\n");
    out
}
