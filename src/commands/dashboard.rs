use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::cli::{DashboardExportArgs, DashboardServeArgs};
use crate::git::Git;
use crate::transcript::{Transcript, TranscriptStore};

#[derive(Debug, Clone, Serialize)]
struct CommitMeta {
    sha: String,
    author_name: String,
    author_email: String,
    author_date_iso: String,
    subject: String,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardEntry {
    commit: CommitMeta,
    transcript: Transcript,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardExport {
    schema_version: String,
    generated_at: DateTime<Utc>,
    repo_id: String,
    entries: Vec<DashboardEntry>,
}

pub(crate) fn cmd_dashboard_export(git: &Git, args: DashboardExportArgs) -> Result<u8> {
    let store = TranscriptStore::git_notes();
    let mut entries = Vec::new();
    for sha in list_note_commits(git).unwrap_or_default() {
        let meta = match commit_meta(git, &sha) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("aigit: dashboard: skipping {sha}: failed to read commit metadata: {e}");
                continue;
            }
        };
        let mut t = match store.load(&git.repo, &sha) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("aigit: dashboard: skipping {sha}: failed to load transcript: {e}");
                continue;
            }
        };
        t.commit = Some(sha.clone());
        if !args.include_answers {
            t.answers.answers.clear();
        }
        entries.push(DashboardEntry { commit: meta, transcript: t });
    }

    entries.sort_by(|a, b| b.commit.author_date_iso.cmp(&a.commit.author_date_iso));
    if let Some(limit) = args.limit {
        entries.truncate(limit);
    }

    let export = DashboardExport {
        schema_version: "aigit-dashboard/0.1".to_string(),
        generated_at: Utc::now(),
        repo_id: git.repo.workdir.to_string_lossy().to_string(),
        entries,
    };

    let out_path = PathBuf::from(args.out);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&export)?;
    std::fs::write(&out_path, json)
        .with_context(|| format!("failed to write {}", out_path.display()))?;

    eprintln!("aigit: dashboard: wrote {}", out_path.display());
    Ok(0)
}

pub(crate) fn cmd_dashboard_serve(git: &Git, args: DashboardServeArgs) -> Result<u8> {
    let dir = git.repo.workdir.join(args.dir);
    let dir = dir
        .canonicalize()
        .with_context(|| format!("failed to resolve dashboard dir {}", dir.display()))?;

    let bind = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind).with_context(|| format!("failed to bind {bind}"))?;
    eprintln!(
        "aigit: dashboard: serving {} on http://{bind}",
        dir.display()
    );
    eprintln!("aigit: dashboard: press Ctrl+C to stop");

    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(s) => s,
            Err(e) => {
                eprintln!("aigit: dashboard: accept failed: {e}");
                continue;
            }
        };
        let dir = dir.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle_http(&mut stream, &dir) {
                eprintln!("aigit: dashboard: request error: {e}");
            }
        });
    }

    Ok(0)
}

fn list_note_commits(git: &Git) -> Result<Vec<String>> {
    let out = std::process::Command::new("git")
        .current_dir(&git.repo.workdir)
        .args(["notes", "--ref=aigit", "list"])
        .output()
        .context("failed to run git notes list")?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let raw = String::from_utf8(out.stdout)?;
    let mut commits = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let _note_sha = parts.next();
        let commit_sha = parts.next();
        if let Some(c) = commit_sha {
            commits.push(c.to_string());
        }
    }
    Ok(commits)
}

fn commit_meta(git: &Git, sha: &str) -> Result<CommitMeta> {
    let out = std::process::Command::new("git")
        .current_dir(&git.repo.workdir)
        .args([
            "show",
            "-s",
            "--date=iso-strict",
            "--format=%H%x09%an%x09%ae%x09%ad%x09%s",
            sha,
        ])
        .output()
        .context("failed to run git show")?;
    if !out.status.success() {
        bail!("git show failed");
    }
    let line = String::from_utf8(out.stdout)?.trim_end().to_string();
    let mut parts = line.split('\t');
    let sha = parts.next().unwrap_or("").to_string();
    let author_name = parts.next().unwrap_or("").to_string();
    let author_email = parts.next().unwrap_or("").to_string();
    let author_date_iso = parts.next().unwrap_or("").to_string();
    let subject_parts = parts.collect::<Vec<_>>();
    let subject = subject_parts.join("\t");
    Ok(CommitMeta {
        sha,
        author_name,
        author_email,
        author_date_iso,
        subject,
    })
}

fn handle_http(stream: &mut TcpStream, root: &Path) -> Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).context("failed to read request")?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let raw_path = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        write_response(stream, 405, "text/plain; charset=utf-8", b"Method Not Allowed", method == "HEAD")?;
        return Ok(());
    }

    let mut path = raw_path.split('?').next().unwrap_or("/").to_string();
    if path.is_empty() {
        path = "/".to_string();
    }
    if !path.starts_with('/') {
        path = format!("/{path}");
    }
    let decoded = percent_decode_path(&path);
    let rel = decoded.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let rel = if rel.ends_with('/') {
        format!("{rel}index.html")
    } else {
        rel.to_string()
    };

    let candidate = root.join(rel);
    let candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            write_response(stream, 404, "text/plain; charset=utf-8", b"Not Found", method == "HEAD")?;
            return Ok(());
        }
    };

    if !candidate.starts_with(root) {
        write_response(stream, 403, "text/plain; charset=utf-8", b"Forbidden", method == "HEAD")?;
        return Ok(());
    }

    let body = match std::fs::read(&candidate) {
        Ok(b) => b,
        Err(_) => {
            write_response(stream, 404, "text/plain; charset=utf-8", b"Not Found", method == "HEAD")?;
            return Ok(());
        }
    };

    let ct = content_type_for_path(&candidate);
    write_response(stream, 200, ct, &body, method == "HEAD")?;
    Ok(())
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Result<()> {
    let status_text = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).context("failed to write headers")?;
    if !head_only {
        stream.write_all(body).context("failed to write body")?;
    }
    let _ = stream.flush();
    Ok(())
}

fn content_type_for_path(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn percent_decode_path(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = from_hex(bytes[i + 1]);
            let h2 = from_hex(bytes[i + 2]);
            if let (Some(a), Some(b)) = (h1, h2) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + (b - b'a')),
        b'A'..=b'F' => Some(10 + (b - b'A')),
        _ => None,
    }
}
