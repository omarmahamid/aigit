use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub workdir: PathBuf,
    pub git_dir: PathBuf,
}

impl GitRepo {
    pub fn discover() -> Result<Self> {
        let out = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .context("failed to run git")?;
        if !out.status.success() {
            return Err(anyhow!("git rev-parse failed"));
        }
        let workdir = PathBuf::from(String::from_utf8(out.stdout)?.trim());

        let out = Command::new("git")
            .current_dir(&workdir)
            .args(["rev-parse", "--git-dir"])
            .output()
            .context("failed to run git")?;
        if !out.status.success() {
            return Err(anyhow!("git rev-parse --git-dir failed"));
        }
        let git_dir_raw = String::from_utf8(out.stdout)?.trim().to_string();
        let git_dir = if Path::new(&git_dir_raw).is_absolute() {
            PathBuf::from(git_dir_raw)
        } else {
            workdir.join(git_dir_raw)
        };

        Ok(Self { workdir, git_dir })
    }
}

#[derive(Debug, Clone)]
pub struct Git {
    pub repo: GitRepo,
}

impl Git {
    pub fn new(repo: GitRepo) -> Self {
        Self { repo }
    }

    pub fn diff_staged(&self) -> Result<(String, Vec<String>)> {
        let diff = self.git_output(["diff", "--staged", "--unified=0"])?;
        let files_raw = self.git_output(["diff", "--staged", "--name-only"])?;
        let changed_files = files_raw
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        Ok((diff, changed_files))
    }

    pub fn diff_range(&self, range: &str) -> Result<(String, Vec<String>)> {
        let diff = self.git_output(["diff", "--unified=0", range])?;
        let files_raw = self.git_output(["diff", "--name-only", range])?;
        let changed_files = files_raw
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        Ok((diff, changed_files))
    }

    pub fn patch_id_for_commit(&self, commit: &str) -> Result<String> {
        let diff = self.git_output(["show", "--pretty=format:", "--unified=0", commit])?;
        self.patch_id_from_diff(&diff)
    }

    pub fn patch_id_from_diff_text(&self, diff: &str) -> Result<String> {
        self.patch_id_from_diff(diff)
    }

    fn patch_id_from_diff(&self, diff: &str) -> Result<String> {
        let mut child = Command::new("git")
            .current_dir(&self.repo.workdir)
            .args(["patch-id", "--stable"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to run git patch-id")?;
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().context("failed to open stdin")?;
            stdin.write_all(diff.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(anyhow!("git patch-id failed"));
        }
        let s = String::from_utf8(out.stdout)?;
        let patch_id = s
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow!("git patch-id returned no output"))?;
        Ok(patch_id.to_string())
    }

    pub fn remote_fingerprint(&self) -> Result<Option<String>> {
        let out = Command::new("git")
            .current_dir(&self.repo.workdir)
            .args(["remote", "get-url", "origin"])
            .output();
        let out = match out {
            Ok(o) => o,
            Err(_) => return Ok(None),
        };
        if !out.status.success() {
            return Ok(None);
        }
        let url = String::from_utf8(out.stdout)?.trim().to_string();
        if url.is_empty() {
            return Ok(None);
        }
        Ok(Some(url))
    }

    pub fn rev_parse_head(&self) -> Result<String> {
        Ok(self.git_output(["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub fn resolve_commitish(&self, commitish: &str) -> Result<String> {
        let s = self.git_output(["rev-parse", commitish])?;
        Ok(s.trim().to_string())
    }

    pub fn run_git_commit(&self, message: Option<&str>, extra_args: &[String]) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo.workdir)
            .env("AIGIT_ALLOW_COMMIT", "1")
            .arg("commit");
        if let Some(msg) = message {
            cmd.args(["-m", msg]);
        }
        cmd.args(extra_args);
        let status = cmd.status().context("failed to run git commit")?;
        if !status.success() {
            return Err(anyhow!("git commit failed"));
        }
        Ok(())
    }

    pub fn install_pre_commit_hook(&self, force: bool) -> Result<()> {
        let hooks_dir = self.repo.git_dir.join("hooks");
        std::fs::create_dir_all(&hooks_dir)?;
        let hook_path = hooks_dir.join("pre-commit");
        if hook_path.exists() && !force {
            return Err(anyhow!(
                "hook already exists at {} (use --force to overwrite)",
                hook_path.display()
            ));
        }
        let script = r#"#!/bin/sh
set -e

if [ -z "$AIGIT_ALLOW_COMMIT" ]; then
  echo "aigit: commit blocked. Use: aigit commit"
  exit 1
fi
"#;
        std::fs::write(&hook_path, script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&hook_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_path, perms)?;
        }
        eprintln!("installed pre-commit hook at {}", hook_path.display());
        Ok(())
    }

    fn git_output<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let out = Command::new("git")
            .current_dir(&self.repo.workdir)
            .args(args)
            .output()
            .context("failed to run git")?;
        if !out.status.success() {
            return Err(anyhow!(
                "git command failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8(out.stdout)?)
    }
}
