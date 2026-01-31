use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::Policy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionHit {
    pub pattern: String,
    pub count: u32,
}

pub fn redact_diff(policy: &Policy, diff: &str) -> Result<(String, Vec<RedactionHit>)> {
    let mut patterns: Vec<(String, Regex)> = Vec::new();

    // built-in patterns (conservative)
    patterns.push((
        "private_key_block".to_string(),
        Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")?,
    ));
    patterns.push((
        "aws_access_key_id".to_string(),
        Regex::new(r"AKIA[0-9A-Z]{16}")?,
    ));
    patterns.push((
        "github_pat".to_string(),
        Regex::new(r"ghp_[A-Za-z0-9]{20,}")?,
    ));
    patterns.push((
        "bearer_token".to_string(),
        Regex::new(r"(?i)bearer\\s+[A-Za-z0-9\\-\\._=]+")?,
    ));

    for (i, pat) in policy.redactions.iter().enumerate() {
        patterns.push((format!("policy_redaction_{i}"), Regex::new(pat)?));
    }

    let mut redacted = diff.to_string();
    let mut hits: Vec<RedactionHit> = Vec::new();
    for (name, re) in patterns {
        let mut count: u32 = 0;
        redacted = re
            .replace_all(&redacted, |_: &regex::Captures| {
                count += 1;
                "[REDACTED]"
            })
            .to_string();
        if count > 0 {
            hits.push(RedactionHit {
                pattern: name,
                count,
            });
        }
    }
    Ok((redacted, hits))
}
