use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use predicates::prelude::*;

fn tmp_repo() -> std::path::PathBuf {
    tempfile::Builder::new()
        .prefix("aigit-test-")
        .tempdir()
        .unwrap()
        .keep()
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn make_mock_codex(dir: &std::path::Path, fixed_score: f64) -> std::path::PathBuf {
    let path = dir.join("mock-codex");
    let script = format!(
        r#"#!/bin/sh
set -e

out=""
schema=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-schema)
      schema="$2"
      shift 2
      ;;
    --output-last-message|-o)
      out="$2"
      shift 2
      ;;
    *)
      shift 1
      ;;
  esac
done

if [ -z "$out" ]; then
  echo "missing --output-last-message" >&2
  exit 2
fi

if [ -z "$schema" ]; then
  echo "missing --output-schema" >&2
  exit 2
fi

if grep -q '"title"[[:space:]]*:[[:space:]]*"aigit.Exam"' "$schema"; then
  cat > "$out" <<'JSON'
{{
  "protocol_version": "aigit/0.1",
  "questions": [
    {{ "id": "change_summary", "category": "summary", "prompt": "What changed in hello.txt and why?", "choices": null }},
    {{ "id": "intent", "category": "intent", "prompt": "Which requirement does adding hello.txt satisfy?", "choices": ["Create a baseline file in the repo", "Migrate the database", "Rotate auth keys", "Increase GPU utilization"] }},
    {{ "id": "invariants", "category": "invariants", "prompt": "Which invariant must remain true about hello.txt?", "choices": ["It stays plain text", "It becomes JSON", "It contains secrets", "It is deleted"] }},
    {{ "id": "risk", "category": "risk", "prompt": "What is the most likely risk of this change?", "choices": ["Break scripts reading initial content", "DB migration failure", "Auth outage", "GPU driver crash"] }},
    {{ "id": "testing", "category": "testing", "prompt": "What testing is appropriate here?", "choices": null }},
    {{ "id": "rollback", "category": "rollback", "prompt": "How do you rollback?", "choices": null }},
    {{ "id": "alternatives", "category": "alternatives", "prompt": "What alternative approach exists and why not chosen?", "choices": null }},
    {{ "id": "security_privacy", "category": "security", "prompt": "Any security/privacy concerns?", "choices": null }}
  ]
}}
JSON
  exit 0
fi

cat > "$out" <<'JSON'
{{
  "total_score": {fixed_score},
  "per_question": [
    {{ "id": "change_summary", "category": "summary", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "intent", "category": "intent", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "invariants", "category": "invariants", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "risk", "category": "risk", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "testing", "category": "testing", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "rollback", "category": "rollback", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "alternatives", "category": "alternatives", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }},
    {{ "id": "security_privacy", "category": "security", "score": {fixed_score}, "completeness": 1.0, "specificity": 1.0, "notes": [] }}
  ],
  "hallucination_flags": []
}}
JSON
"#
    );
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path
}

#[test]
fn exam_json_emits_questions() {
    let dir = tmp_repo();
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "user.name", "Test User"]);

    fs::write(dir.join("foo.txt"), "hello\n").unwrap();
    git(&dir, &["add", "foo.txt"]);

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    cmd.current_dir(&dir).args(["exam", "--format", "json"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"protocol_version\""))
        .stdout(predicate::str::contains("\"questions\""));
}

#[test]
fn exam_grades_via_codex_cli_when_enabled() {
    let dir = tmp_repo();
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "user.name", "Test User"]);

    fs::write(dir.join("foo.txt"), "hello\n").unwrap();
    git(&dir, &["add", "foo.txt"]);

    let mock_codex = make_mock_codex(&dir, 0.95);
    fs::write(
        dir.join(".aigit.toml"),
        format!(
            r#"
provider = "codex-cli"
model = "gpt-5-codex"

[codex_cli]
command = "{}"
sandbox = "read-only"
timeout_secs = 5
"#,
            mock_codex.display()
        ),
    )
    .unwrap();

    let mut answers = BTreeMap::new();
    for (id, text) in [
        ("change_summary", "Updated foo.txt."),
        ("intent", "Meets requirement."),
        ("invariants", "Assumes foo.txt exists."),
        ("risk", "Minimal risk."),
        ("testing", "N/A."),
        ("rollback", "git revert."),
        ("alternatives", "No alternatives."),
        ("security_privacy", "No secrets."),
    ] {
        answers.insert(id.to_string(), text.to_string());
    }
    let answers_path = dir.join("answers.json");
    fs::write(
        &answers_path,
        serde_json::to_string_pretty(&serde_json::json!({ "answers": answers })).unwrap(),
    )
    .unwrap();

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    cmd.current_dir(&dir).args([
        "exam",
        "--format",
        "json",
        "--answers",
        answers_path.to_str().unwrap(),
    ]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let transcript: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        transcript["provider"]["provider"].as_str().unwrap(),
        "codex-cli"
    );
    let total = transcript["score"]["total_score"].as_f64().unwrap();
    assert!((total - 0.95).abs() < 1e-9, "expected 0.95, got {total}");

    // Also verify that exam generation is dynamic (comes from codex-cli) and can include choices.
    let mut packet = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    packet.current_dir(&dir)
        .args(["exam", "--format", "json"]);
    let out = packet.assert().success().get_output().stdout.clone();
    let packet_json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let questions = packet_json["exam"]["questions"].as_array().unwrap();
    assert!(questions.iter().any(|q| q.get("choices").is_some()));
}

#[test]
fn verify_passes_with_matching_transcript_note() {
    let dir = tmp_repo();
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "user.name", "Test User"]);

    // Base commit
    fs::write(dir.join("foo.txt"), "v1\n").unwrap();
    git(&dir, &["add", "foo.txt"]);
    git(&dir, &["commit", "-m", "base"]);

    // Change commit
    fs::write(dir.join("foo.txt"), "v2\n").unwrap();
    git(&dir, &["add", "foo.txt"]);
    git(&dir, &["commit", "-m", "change"]);

    // Generate a passing transcript for HEAD~1..HEAD
    let mut answers = BTreeMap::new();
    for (id, text) in [
        ("change_summary", "Updated foo.txt to change behavior; foo.txt."),
        ("intent", "Meets requirement to update output in foo.txt."),
        ("invariants", "Assumes foo.txt exists and remains plain text."),
        (
            "risk",
            "Risk: regression in downstream parsing; could break consumers; failure would surface on read.",
        ),
        ("testing", "Ran `cargo test` (N/A for txt); should add integration coverage; test keyword."),
        ("rollback", "Rollback by `git revert` the commit; mitigation via quick backout."),
        ("alternatives", "Alternative: new file; rejected to keep change minimal."),
        ("security_privacy", "No secrets/PII; no auth/authz changes."),
    ] {
        answers.insert(id.to_string(), text.to_string());
    }
    let answers_path = dir.join("answers.json");
    fs::write(
        &answers_path,
        serde_json::to_string_pretty(&serde_json::json!({ "answers": answers })).unwrap(),
    )
    .unwrap();

    let mut exam = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    exam.current_dir(&dir).args([
        "exam",
        "--format",
        "json",
        "--range",
        "HEAD~1..HEAD",
        "--answers",
        answers_path.to_str().unwrap(),
    ]);
    let output = exam.assert().success().get_output().stdout.clone();

    // Attach transcript to HEAD via git notes ref=aigit
    let transcript = String::from_utf8(output).unwrap();
    git(
        &dir,
        &[
            "notes",
            "--ref=aigit",
            "add",
            "-f",
            "-m",
            &transcript,
            "HEAD",
        ],
    );

    let mut verify = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    verify.current_dir(&dir).args(["verify", "HEAD"]);
    verify
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn policy_validate_succeeds() {
    let dir = tmp_repo();
    git(&dir, &["init"]);

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    cmd.current_dir(&dir).args(["policy", "validate"]);
    cmd.assert().success();
}

#[test]
fn config_set_writes_policy_file() {
    let dir = tmp_repo();
    git(&dir, &["init"]);

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    cmd.current_dir(&dir)
        .args(["config", "set", "exam_mode", "json"]);
    cmd.assert().success();

    let raw = fs::read_to_string(dir.join(".aigit.toml")).unwrap();
    assert!(
        raw.contains("exam_mode = \"json\""),
        "expected exam_mode in .aigit.toml, got:\n{raw}"
    );
}

#[test]
fn install_hook_creates_pre_commit_hook() {
    let dir = tmp_repo();
    git(&dir, &["init"]);

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("aigit"));
    cmd.current_dir(&dir).args(["install-hook"]);
    cmd.assert().success();

    let hook_path = dir.join(".git").join("hooks").join("pre-commit");
    let raw = fs::read_to_string(&hook_path).unwrap();
    assert!(
        raw.contains("aigit: commit blocked"),
        "expected pre-commit hook content, got:\n{raw}"
    );
}
