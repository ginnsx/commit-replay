use serde::Serialize;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize)]
pub struct GitRepoInfo {
    pub branch: String,
    pub branches: Vec<String>,
    pub remote_url: Option<String>,
}

fn run_git(repo_path: &str, args: &[&str]) -> Result<String> {
    let output = crate::process::command("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .map_err(|e| AppError::Vcs(format!("failed to spawn git: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    Err(AppError::Vcs(format!(
        "git {} failed: {stderr}{stdout}",
        args.first().copied().unwrap_or("")
    )))
}

pub fn probe_git_repo(repo_path: &str) -> Result<GitRepoInfo> {
    let inside = run_git(repo_path, &["rev-parse", "--is-inside-work-tree"])?
        .trim()
        .to_string();
    if inside != "true" {
        return Err(AppError::Vcs("not a git working tree".into()));
    }

    let mut branch = run_git(repo_path, &["branch", "--show-current"])?
        .trim()
        .to_string();
    if branch.is_empty() {
        branch = run_git(repo_path, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_string();
    }

    let branches_out = run_git(repo_path, &["branch", "--format=%(refname:short)"])?;
    let mut branches: Vec<String> = branches_out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    branches.sort();
    if !branch.is_empty() && !branches.iter().any(|b| b == &branch) {
        branches.insert(0, branch.clone());
    }
    if branches.is_empty() && !branch.is_empty() {
        branches.push(branch.clone());
    }

    let remote_url = run_git(repo_path, &["remote", "get-url", "origin"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(GitRepoInfo {
        branch,
        branches,
        remote_url,
    })
}
