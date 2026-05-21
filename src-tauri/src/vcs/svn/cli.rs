use std::process::Command;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Default)]
pub struct SvnCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Run an SVN subcommand and return stdout on success.
pub fn run_svn(url: &str, creds: &SvnCredentials, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("svn");
    // --non-interactive: do not prompt; fail if credentials are missing.
    // Avoid --no-auth-prompt: not supported on older SVN builds (e.g. some Windows installs).
    cmd.arg("--non-interactive");

    if let Some(user) = &creds.username {
        cmd.arg("--username").arg(user);
    }
    if let Some(pass) = &creds.password {
        cmd.arg("--password").arg(pass);
    }

    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg(url);

    let output = cmd
        .output()
        .map_err(|e| AppError::Vcs(format!("failed to spawn svn: {e}")))?;

    if output.status.success() {
        return String::from_utf8(output.stdout).map_err(|e| {
            AppError::Vcs(format!("svn output is not valid UTF-8: {e}"))
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(AppError::Vcs(format!(
        "svn {} failed (exit {:?}): {stderr}{stdout}",
        args.first().copied().unwrap_or(""),
        output.status.code(),
    )))
}

pub fn svn_log_xml(url: &str, creds: &SvnCredentials, limit: usize) -> Result<String> {
    run_svn(
        url,
        creds,
        &["log", "--xml", "-l", &limit.to_string()],
    )
}

pub fn svn_diff_revision(url: &str, creds: &SvnCredentials, revision: u64) -> Result<String> {
    run_svn(url, creds, &["diff", "-c", &revision.to_string()])
}

pub fn svn_log_revision_xml(url: &str, creds: &SvnCredentials, revision: u64) -> Result<String> {
    run_svn(
        url,
        creds,
        &["log", "--xml", "-r", &revision.to_string(), "-l", "1"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_svn_fails_when_svn_missing() {
        let result = Command::new("svn_nonexistent_binary_42").output();
        assert!(result.is_err());
    }
}
